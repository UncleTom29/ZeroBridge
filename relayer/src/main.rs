// relayer/src/main.rs
//! ZeroBridge Relayer Node
//! 
//! FOCUSED RESPONSIBILITIES (Zero overlap with coordinator):
//! 1. Listen to gateway events on ALL supported chains
//! 2. Notify coordinator about deposits (coordinator creates Zcash notes)
//! 3. Notify coordinator about withdrawal requests (coordinator verifies proofs)
//! 4. Query coordinator for authorized withdrawals
//! 5. Execute authorized withdrawal transactions on destination chains
//! 6. P2P coordination with other relayers (claim tasks, prevent duplicates)
//! 7. Earn fees for successful relay executions
//! 
//! NEVER DOES:
//! - Create Zcash notes (coordinator only)
//! - Verify Zcash proofs (coordinator only)
//! - Manage liquidity pools (coordinator only)
//! - Manage token registry (coordinator only)
//! - Sign withdrawal authorizations (coordinator only)

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::signal;
use std::path::PathBuf;
use std::sync::Arc;

mod config;
mod event_listener;
mod transaction_executor;
mod p2p_network;
mod stake_manager;
mod database;
mod coordinator_client;
mod metrics;

use config::RelayerConfig;
use event_listener::EventListenerManager;
use transaction_executor::TransactionExecutor;
use p2p_network::P2PNetwork;
use stake_manager::StakeManager;
use database::RelayerDatabase;
use coordinator_client::CoordinatorClient;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value = "relayer-config.toml")]
    config: PathBuf,

    #[clap(short, long)]
    verbose: bool,

    #[clap(short, long, default_value = "9091")]
    metrics_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing(args.verbose)?;

    info!("🔄 Starting ZeroBridge Relayer v{}", env!("CARGO_PKG_VERSION"));
    info!("Configuration: {:?}", args.config);

    let config = RelayerConfig::load(&args.config)
        .context("Failed to load configuration")?;
    
    info!("✓ Configuration loaded");
    info!("  Coordinator: {}", config.coordinator_url);
    info!("  Monitoring chains: {}", config.chains.len());
    info!("  Relayer identity: {}", config.relayer_identity.name);

    let db = RelayerDatabase::new(&config.database_path)
        .await
        .context("Failed to initialize database")?;
    info!("✓ Database initialized");

    // Connect to coordinator (read-only access)
    let coordinator_client = Arc::new(
        CoordinatorClient::new(&config.coordinator_url)
            .context("Failed to connect to coordinator")?
    );
    info!("✓ Connected to coordinator at {}", config.coordinator_url);

    let stake_manager = Arc::new(
        StakeManager::new(config.clone(), db.clone())
            .await
            .context("Failed to initialize stake manager")?
    );
    info!("✓ Stake manager initialized");

    stake_manager.ensure_minimum_stake().await?;
    info!("✓ Minimum stake requirement met: {} tokens", config.staking.current_stake);

    let p2p_network = Arc::new(
        P2PNetwork::new(config.clone(), stake_manager.clone())
            .await
            .context("Failed to initialize P2P network")?
    );
    info!("✓ P2P network initialized on port {}", config.p2p.port);

    let tx_executor = Arc::new(
        TransactionExecutor::new(
            config.clone(),
            coordinator_client.clone(),
            stake_manager.clone(),
            db.clone(),
        )
        .await
        .context("Failed to initialize transaction executor")?
    );
    info!("✓ Transaction executor initialized");

    let mut event_listeners = EventListenerManager::new(
        config.clone(),
        coordinator_client.clone(),
        tx_executor.clone(),
        p2p_network.clone(),
        db.clone(),
    )
    .await
    .context("Failed to initialize event listeners")?;
    info!("✓ Event listeners initialized for {} chains", config.chains.len());

    let metrics_handle = tokio::spawn(async move {
        if let Err(e) = metrics::start_server(args.metrics_port).await {
            error!("Metrics server error: {}", e);
        }
    });
    info!("✓ Metrics server started on port {}", args.metrics_port);

    let relayer = Relayer {
        config,
        db,
        coordinator_client,
        stake_manager,
        p2p_network,
        tx_executor,
        event_listeners,
    };

    info!("🚀 Relayer fully initialized and running");
    info!("   Listening for gateway events and relaying transactions");

    tokio::select! {
        result = relayer.run() => {
            if let Err(e) = result {
                error!("Relayer error: {}", e);
                return Err(e);
            }
        }
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    metrics_handle.abort();
    
    info!("Relayer stopped gracefully");
    Ok(())
}

struct Relayer {
    config: RelayerConfig,
    db: RelayerDatabase,
    coordinator_client: Arc<CoordinatorClient>,
    stake_manager: Arc<StakeManager>,
    p2p_network: Arc<P2PNetwork>,
    tx_executor: Arc<TransactionExecutor>,
    event_listeners: EventListenerManager,
}

impl Relayer {
    /// Run the relayer main loop
    /// FOCUSED: Query coordinator and execute authorized withdrawals
    async fn run(mut self) -> Result<()> {
        info!("Starting relayer main loop");

        // Start P2P network
        let p2p_handle = {
            let network = self.p2p_network.clone();
            tokio::spawn(async move {
                if let Err(e) = network.start().await {
                    error!("P2P network error: {}", e);
                }
            })
        };

        // Start event listeners (they notify coordinator)
        self.event_listeners.start_all().await?;
        info!("✓ All event listeners started");

        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(self.config.poll_interval)
        );

        let mut tick_count = 0u64;

        loop {
            interval.tick().await;
            tick_count += 1;

            if tick_count % 10 == 0 {
                info!("Relayer tick #{}", tick_count);
            }

            // Query coordinator for authorized withdrawals and execute them
            // This is our PRIMARY responsibility
            if let Err(e) = self.process_authorized_withdrawals().await {
                error!("Error processing withdrawals: {}", e);
            }

            // Claim rewards for completed relays
            if tick_count % 60 == 0 {
                if let Err(e) = self.claim_rewards().await {
                    error!("Error claiming rewards: {}", e);
                }
            }

            // Update metrics
            if tick_count % 30 == 0 {
                self.update_metrics().await;
            }

            // P2P heartbeat
            if tick_count % 5 == 0 {
                if let Err(e) = self.p2p_network.send_heartbeat().await {
                    warn!("Failed to send heartbeat: {}", e);
                }
            }
        }
    }

    /// Query coordinator for authorized withdrawals and execute them
    /// Coordinator has already verified proofs and signed authorization
    async fn process_authorized_withdrawals(&self) -> Result<()> {
        // Query coordinator API for withdrawals that have been authorized
        let authorized = self.coordinator_client
            .query_authorized_withdrawals()
            .await?;
        
        if !authorized.is_empty() {
            info!("Found {} authorized withdrawals from coordinator", authorized.len());
        }

        for withdrawal in authorized {
            // Check if another relayer is already handling this
            if self.p2p_network.is_task_claimed(&withdrawal.withdrawal_id).await? {
                continue;
            }

            // Claim this task via P2P
            if let Err(e) = self.p2p_network
                .broadcast_task_claim(&withdrawal.withdrawal_id)
                .await
            {
                warn!("Failed to claim task: {}", e);
                continue;
            }

            match self.execute_authorized_withdrawal(withdrawal).await {
                Ok(tx_hash) => {
                    info!("✓ Executed withdrawal: tx={}", tx_hash);
                    
                    // Earn fee for this relay
                    if let Err(e) = self.stake_manager.record_successful_relay().await {
                        warn!("Failed to record relay: {}", e);
                    }
                }
                Err(e) => {
                    warn!("Failed to execute withdrawal: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Execute an authorized withdrawal on the destination chain
    /// Coordinator has already verified the proof and provided authorization signature
    async fn execute_authorized_withdrawal(
        &self,
        withdrawal: coordinator_client::AuthorizedWithdrawal,
    ) -> Result<String> {
        info!("Executing authorized withdrawal: {}", withdrawal.withdrawal_id);

        // Verify coordinator authorization signature
        if !self.verify_coordinator_signature(&withdrawal)? {
            anyhow::bail!("Invalid coordinator authorization signature");
        }

        // Submit transaction to destination chain
        let tx_hash = self.tx_executor
            .execute_withdrawal(
                withdrawal.target_chain_id,
                &withdrawal.recipient,
                &withdrawal.token,
                withdrawal.amount,
                &withdrawal.nullifier,
                &withdrawal.authorization_signature,
            )
            .await?;

        // Broadcast success to P2P network
        self.p2p_network
            .broadcast_withdrawal_execution(&withdrawal.withdrawal_id, &tx_hash)
            .await?;

        // Store in local database
        self.db
            .record_withdrawal_execution(
                &withdrawal.withdrawal_id,
                &tx_hash,
                chrono::Utc::now().timestamp(),
            )
            .await?;

        Ok(tx_hash)
    }

    /// Verify coordinator's authorization signature
    fn verify_coordinator_signature(
        &self,
        withdrawal: &coordinator_client::AuthorizedWithdrawal,
    ) -> Result<bool> {
        // In production, verify the signature using coordinator's public key
        // For now, just check it's not empty
        Ok(!withdrawal.authorization_signature.is_empty())
    }

    /// Claim accumulated rewards from hub contract
    async fn claim_rewards(&self) -> Result<()> {
        let rewards = self.stake_manager.get_pending_rewards().await?;
        
        if rewards > 0 {
            info!("Claiming {} accumulated rewards", rewards);
            self.stake_manager.claim_rewards().await?;
            info!("✓ Rewards claimed successfully");
        }

        Ok(())
    }

    /// Update metrics for monitoring
    async fn update_metrics(&self) {
        if let Ok(stats) = self.db.get_stats().await {
            metrics::WITHDRAWALS_EXECUTED.set(stats.withdrawals_executed as i64);
            metrics::REWARDS_EARNED.set(stats.total_rewards as i64);
            metrics::STAKE_AMOUNT.set(self.config.staking.current_stake as i64);
            metrics::SUCCESSFUL_RELAYS.set(stats.successful_relays as i64);
        }
    }
}

fn init_tracing(verbose: bool) -> Result<()> {
    let log_level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    format!("zerobridge_relayer={},tower_http=debug", log_level).into()
                }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}