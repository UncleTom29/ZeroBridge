// zcash-coordinator/src/main.rs
//! ZeroBridge Zcash Coordinator
//! 
//! FOCUSED RESPONSIBILITIES:
//! 1. Create Zcash shielded notes for deposits
//! 2. Verify Zcash proofs for withdrawals
//! 3. Manage token registry (canonical token mappings)
//! 4. Manage liquidity pools
//! 5. Authorize and SIGN withdrawals
//! 6. Maintain merkle tree state
//! 
//! NOT RESPONSIBLE FOR:
//! - Listening to gateway events (relayer does this)
//! - Submitting transactions to gateways (relayer does this)
//! - P2P coordination (relayer does this)

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::signal;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

mod config;
mod shielded_pool;
mod token_registry;
mod liquidity_manager;
mod database;
mod rpc_server;
mod zcash_client;

use config::Config;
use shielded_pool::ShieldedPoolManager;
use token_registry::TokenRegistry;
use liquidity_manager::LiquidityManager;
use database::Database;
use rpc_server::RpcServer;
use zcash_client::ZcashClient;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value = "config.toml")]
    config: PathBuf,

    #[clap(short, long)]
    verbose: bool,

    #[clap(short, long, default_value = "8080")]
    port: u16,

    #[clap(short, long, default_value = "coordinator.db")]
    database: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing(args.verbose)?;

    info!("🌉 Starting ZeroBridge Zcash Coordinator v{}", env!("CARGO_PKG_VERSION"));
    info!("Configuration file: {:?}", args.config);

    let config = Config::load(&args.config)
        .context("Failed to load configuration")?;
    
    info!("✓ Configuration loaded successfully");
    info!("  Zcash network: {:?}", config.zcash.network);

    // Initialize database
    let db = Database::new(&args.database)
        .await
        .context("Failed to initialize database")?;
    info!("✓ Database initialized");

    // Initialize Zcash client
    let zcash_client = ZcashClient::new(config.zcash.clone())
        .await
        .context("Failed to connect to Zcash node")?;
    info!("✓ Connected to Zcash node at {}", config.zcash.rpc_url);

    // Wait for Zcash node to sync
    info!("Waiting for Zcash node synchronization...");
    zcash_client.wait_for_sync().await?;
    info!("✓ Zcash node synchronized");

    // Initialize token registry
    let token_registry = Arc::new(
        TokenRegistry::load(&config.tokens_config)
            .await
            .context("Failed to load token registry")?
    );
    info!("✓ Token registry loaded ({} tokens)", token_registry.token_count());

    // Initialize liquidity manager
    let liquidity_manager = Arc::new(RwLock::new(
        LiquidityManager::new(db.clone(), config.liquidity.clone())
            .await
            .context("Failed to initialize liquidity manager")?
    ));

    // Initialize shielded pool manager
    let shielded_pool = Arc::new(RwLock::new(
        ShieldedPoolManager::new(
            zcash_client.clone(),
            db.clone(),
            token_registry.clone(),
            liquidity_manager.clone(),
        )
            .await
            .context("Failed to initialize shielded pool")?
    ));
    info!("✓ Shielded pool manager initialized");
    info!("✓ Liquidity manager initialized");

    // Start RPC server for relayer queries
    let rpc_server = RpcServer::new(
        args.port,
        db.clone(),
        shielded_pool.clone(),
        token_registry.clone(),
        liquidity_manager.clone(),
    );
    
    let rpc_handle = tokio::spawn(async move {
        if let Err(e) = rpc_server.start().await {
            error!("RPC server error: {}", e);
        }
    });
    info!("✓ RPC server started on port {}", args.port);

    // Create coordinator instance
    let coordinator = Coordinator {
        config,
        db,
        zcash_client,
        shielded_pool,
        token_registry,
        liquidity_manager,
    };

    info!("🚀 Coordinator fully initialized and running");
    info!("   Relayers can connect to process deposits and withdrawals");

    // Run coordinator with graceful shutdown
    tokio::select! {
        result = coordinator.run() => {
            if let Err(e) = result {
                error!("Coordinator error: {}", e);
                return Err(e);
            }
        }
        _ = signal::ctrl_c() => {
            info!("Received shutdown signal, stopping coordinator...");
        }
    }

    rpc_handle.abort();
    
    info!("Coordinator stopped gracefully");
    Ok(())
}

struct Coordinator {
    config: Config,
    db: Database,
    zcash_client: ZcashClient,
    shielded_pool: Arc<RwLock<ShieldedPoolManager>>,
    token_registry: Arc<TokenRegistry>,
    liquidity_manager: Arc<RwLock<LiquidityManager>>,
}

impl Coordinator {
    /// Run the coordinator main loop
    /// FOCUSED: Only processes deposits/withdrawals notified by relayers
    async fn run(self) -> Result<()> {
        info!("Starting coordinator main loop");

        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(self.config.poll_interval)
        );

        let mut tick_count = 0u64;

        loop {
            interval.tick().await;
            tick_count += 1;

            if tick_count % 10 == 0 {
                info!("Coordinator tick #{}", tick_count);
            }

            // Process pending deposits (create Zcash notes)
            // These are deposits that relayers have notified us about
            if let Err(e) = self.process_deposits().await {
                error!("Error processing deposits: {}", e);
            }

            // Process pending withdrawals (verify proofs, authorize)
            // These are withdrawals that relayers have notified us about
            if let Err(e) = self.process_withdrawals().await {
                error!("Error processing withdrawals: {}", e);
            }

            // Update Zcash state
            if let Err(e) = self.sync_zcash_state().await {
                error!("Error syncing Zcash state: {}", e);
            }

            // Rebalance liquidity if needed
            if tick_count % 60 == 0 {
                if let Err(e) = self.rebalance_liquidity().await {
                    error!("Error rebalancing liquidity: {}", e);
                }
            }

            // Update metrics
            if tick_count % 30 == 0 {
                self.update_metrics().await;
            }
        }
    }

    /// Process pending deposits from database (populated by relayer notifications)
    async fn process_deposits(&self) -> Result<()> {
        let pending = self.db.get_pending_deposits().await?;
        
        if !pending.is_empty() {
            info!("Processing {} pending deposits", pending.len());
        }

        for deposit in pending {
            match self.handle_deposit(deposit).await {
                Ok(_) => {
                    info!("✓ Processed deposit: {}", deposit.deposit_id);
                }
                Err(e) => {
                    warn!("Failed to process deposit {}: {}", deposit.deposit_id, e);
                }
            }
        }

        Ok(())
    }

    /// Handle a single deposit - create Zcash note
    async fn handle_deposit(&self, deposit: database::Deposit) -> Result<()> {
        info!("Handling deposit: {} ({} -> chain {})", 
            deposit.deposit_id, deposit.amount, deposit.target_chain_id);

        // 1. Verify liquidity on destination chain
        let token_info = self.token_registry
            .get_token_for_chain(deposit.target_chain_id, &deposit.token)
            .context("Token not found in registry")?;

        {
            let liquidity_manager = self.liquidity_manager.read().await;
            liquidity_manager
                .ensure_liquidity(
                    deposit.target_chain_id,
                    &token_info.address,
                    deposit.amount,
                )
                .await
                .context("Insufficient liquidity on destination chain")?;
        }

        // 2. Create Zcash shielded note
        let (note_commitment, zcash_txid) = {
            let mut shielded_pool = self.shielded_pool.write().await;
            shielded_pool
                .create_deposit_note(
                    deposit.source_chain_id,
                    &deposit.token,
                    deposit.amount,
                    &deposit.recipient,
                    &deposit.zcash_address,
                )
                .await
                .context("Failed to create Zcash shielded note")?
        };

        info!("Created Zcash note: commitment={:?}, txid={}", 
            note_commitment, zcash_txid);

        // 3. Lock liquidity for this deposit
        {
            let mut liquidity_manager = self.liquidity_manager.write().await;
            liquidity_manager
                .lock_liquidity(
                    deposit.target_chain_id,
                    &token_info.address,
                    deposit.amount,
                )
                .await?;
        }

        // 4. Update database
        self.db
            .mark_deposit_processed(
                &deposit.deposit_id,
                &hex::encode(note_commitment),
                &zcash_txid,
            )
            .await?;

        info!("✓ Deposit processed successfully");
        Ok(())
    }

    /// Process pending withdrawals - verify proofs and authorize
    async fn process_withdrawals(&self) -> Result<()> {
        let pending = self.db.get_pending_withdrawals().await?;
        
        if !pending.is_empty() {
            info!("Processing {} pending withdrawals", pending.len());
        }

        for withdrawal in pending {
            match self.handle_withdrawal(withdrawal).await {
                Ok(_) => {
                    info!("✓ Processed withdrawal: {}", withdrawal.withdrawal_id);
                }
                Err(e) => {
                    warn!("Failed to process withdrawal {}: {}", 
                        withdrawal.withdrawal_id, e);
                }
            }
        }

        Ok(())
    }

    /// Handle a single withdrawal - verify proof and authorize with signature
    async fn handle_withdrawal(&self, withdrawal: database::Withdrawal) -> Result<()> {
        info!("Handling withdrawal: {} (amount: {})", 
            withdrawal.withdrawal_id, withdrawal.amount);

        // 1. Verify Zcash proof and nullifier
        let valid = {
            let shielded_pool = self.shielded_pool.read().await;
            shielded_pool
                .verify_withdrawal_proof(
                    &withdrawal.nullifier,
                    &withdrawal.zcash_proof,
                    &withdrawal.merkle_root,
                    withdrawal.amount,
                )
                .await
                .context("Proof verification failed")?
        };

        if !valid {
            warn!("Invalid proof for withdrawal: {}", withdrawal.withdrawal_id);
            self.db
                .mark_withdrawal_invalid(&withdrawal.withdrawal_id, "Invalid proof")
                .await?;
            return Ok(());
        }

        // 2. Mark nullifier as spent in Zcash
        {
            let shielded_pool = self.shielded_pool.read().await;
            shielded_pool
                .mark_nullifier_spent(&withdrawal.nullifier)
                .await?;
        }

        // 3. Get token info for destination chain
        let token_info = self.token_registry
            .get_token_for_chain(withdrawal.target_chain_id, &withdrawal.token)
            .context("Token not found in registry")?;

        // 4. Generate authorization signature
        let auth_signature = self.generate_withdrawal_signature(
            &withdrawal.withdrawal_id,
            &withdrawal.recipient,
            &token_info.address,
            withdrawal.amount,
            &withdrawal.nullifier,
        )?;

        // 5. Authorize withdrawal in database with signature
        self.db
            .authorize_withdrawal(
                &withdrawal.withdrawal_id,
                &token_info.address,
                withdrawal.amount,
                &auth_signature,
            )
            .await?;

        // 6. Release locked liquidity
        {
            let mut liquidity_manager = self.liquidity_manager.write().await;
            liquidity_manager
                .release_liquidity(
                    withdrawal.target_chain_id,
                    &token_info.address,
                    withdrawal.amount,
                )
                .await?;
        }

        info!("✓ Withdrawal authorized with signature - relayer can now execute");
        Ok(())
    }

    /// Generate authorization signature for withdrawal
    /// This proves the coordinator verified the proof and authorizes execution
    fn generate_withdrawal_signature(
        &self,
        withdrawal_id: &str,
        recipient: &str,
        token: &str,
        amount: u64,
        nullifier: &[u8],
    ) -> Result<Vec<u8>> {
        use sha2::{Sha256, Digest};
        
        // Create message to sign
        let mut hasher = Sha256::new();
        hasher.update(withdrawal_id.as_bytes());
        hasher.update(recipient.as_bytes());
        hasher.update(token.as_bytes());
        hasher.update(&amount.to_le_bytes());
        hasher.update(nullifier);
        let message_hash = hasher.finalize();

        // In production, sign with coordinator's private key
        // For now, return the hash as signature
        Ok(message_hash.to_vec())
    }

    /// Sync Zcash blockchain state
    async fn sync_zcash_state(&self) -> Result<()> {
        let info = self.zcash_client.get_blockchain_info().await?;
        
        self.db
            .update_zcash_state(
                info.blocks,
                &info.bestblockhash,
                info.verificationprogress,
            )
            .await?;

        Ok(())
    }

    /// Rebalance liquidity across chains
    async fn rebalance_liquidity(&self) -> Result<()> {
        info!("Checking liquidity rebalancing...");
        
        let liquidity_manager = self.liquidity_manager.read().await;
        let rebalance_needed = liquidity_manager
            .check_rebalancing_needed()
            .await?;

        if !rebalance_needed.is_empty() {
            info!("Rebalancing needed for {} pools", rebalance_needed.len());
            
            drop(liquidity_manager);
            let mut liquidity_manager = self.liquidity_manager.write().await;
            
            for (chain_id, token) in rebalance_needed {
                if let Err(e) = liquidity_manager
                    .trigger_rebalance(chain_id, &token)
                    .await
                {
                    warn!("Failed to rebalance {}/{}: {}", chain_id, token, e);
                }
            }
        }

        Ok(())
    }

    /// Update metrics
    async fn update_metrics(&self) {
        if let Ok(stats) = self.db.get_stats().await {
            info!("Stats - Deposits: {}, Withdrawals: {}, Volume: {}", 
                stats.total_deposits,
                stats.total_withdrawals,
                stats.total_volume
            );
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
                    format!(
                        "zcash_coordinator={},tower_http=debug,sqlx=warn",
                        log_level
                    )
                    .into()
                }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    Ok(())
}