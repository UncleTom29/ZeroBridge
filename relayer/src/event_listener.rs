// relayer/src/event_listener.rs
//! Event listener for gateway contracts
//! FOCUSED: Monitor events and notify coordinator
//! Does NOT verify proofs or manage liquidity (coordinator's job)

use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::config::{ChainType, RelayerConfig};
use crate::coordinator_client::{CoordinatorClient, DepositNotification};
use crate::transaction_executor::TransactionExecutor;
use crate::p2p_network::P2PNetwork;
use crate::database::RelayerDatabase;

pub struct EventListenerManager {
    listeners: Vec<Box<dyn EventListener>>,
}

impl EventListenerManager {
    pub async fn new(
        config: RelayerConfig,
        coordinator_client: Arc<CoordinatorClient>,
        tx_executor: Arc<TransactionExecutor>,
        p2p_network: Arc<P2PNetwork>,
        db: RelayerDatabase,
    ) -> Result<Self> {
        let mut listeners: Vec<Box<dyn EventListener>> = Vec::new();

        for chain_config in config.chains {
            let listener: Box<dyn EventListener> = match chain_config.chain_type {
                ChainType::Ethereum | ChainType::Base | ChainType::Polygon => {
                    Box::new(
                        EvmEventListener::new(
                            chain_config,
                            coordinator_client.clone(),
                            tx_executor.clone(),
                            p2p_network.clone(),
                            db.clone(),
                        )
                        .await?,
                    )
                }
                ChainType::Solana => {
                    Box::new(
                        SolanaEventListener::new(
                            chain_config,
                            coordinator_client.clone(),
                            tx_executor.clone(),
                            p2p_network.clone(),
                            db.clone(),
                        )
                        .await?,
                    )
                }
                ChainType::Near => {
                    Box::new(
                        NearEventListener::new(
                            chain_config,
                            coordinator_client.clone(),
                            tx_executor.clone(),
                            p2p_network.clone(),
                            db.clone(),
                        )
                        .await?,
                    )
                }
                ChainType::Mina => {
                    Box::new(
                        MinaEventListener::new(
                            chain_config,
                            coordinator_client.clone(),
                            tx_executor.clone(),
                            p2p_network.clone(),
                            db.clone(),
                        )
                        .await?,
                    )
                }
            };

            listeners.push(listener);
        }

        Ok(Self { listeners })
    }

    pub async fn start_all(&mut self) -> Result<()> {
        for listener in &mut self.listeners {
            listener.start().await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait EventListener: Send + Sync {
    async fn start(&mut self) -> Result<()>;
}

// ============ EVM Event Listener ============

struct EvmEventListener {
    chain_config: crate::config::ChainConfig,
    coordinator_client: Arc<CoordinatorClient>,
    tx_executor: Arc<TransactionExecutor>,
    p2p_network: Arc<P2PNetwork>,
    db: RelayerDatabase,
}

impl EvmEventListener {
    async fn new(
        chain_config: crate::config::ChainConfig,
        coordinator_client: Arc<CoordinatorClient>,
        tx_executor: Arc<TransactionExecutor>,
        p2p_network: Arc<P2PNetwork>,
        db: RelayerDatabase,
    ) -> Result<Self> {
        Ok(Self {
            chain_config,
            coordinator_client,
            tx_executor,
            p2p_network,
            db,
        })
    }
}

#[async_trait::async_trait]
impl EventListener for EvmEventListener {
    async fn start(&mut self) -> Result<()> {
        info!(
            "Starting EVM event listener for chain: {}",
            self.chain_config.name
        );

        let chain_id = self.chain_config.chain_id;
        let ws_url = self
            .chain_config
            .ws_url
            .clone()
            .unwrap_or_else(|| self.chain_config.rpc_url.clone());
        let gateway_address = self.chain_config.gateway_address.clone();

        let coordinator = self.coordinator_client.clone();
        let p2p = self.p2p_network.clone();

        tokio::spawn(async move {
            if let Err(e) = Self::listen_loop(
                chain_id,
                &ws_url,
                &gateway_address,
                coordinator,
                p2p,
            )
            .await
            {
                warn!("EVM listener error for chain {}: {}", chain_id, e);
            }
        });

        Ok(())
    }
}

impl EvmEventListener {
    async fn listen_loop(
        chain_id: u64,
        ws_url: &str,
        gateway_address: &str,
        coordinator: Arc<CoordinatorClient>,
        p2p: Arc<P2PNetwork>,
    ) -> Result<()> {
        use ethers::prelude::*;

        let provider = Provider::<Ws>::connect(ws_url).await?;
        let gateway_address: Address = gateway_address.parse()?;

        // Subscribe to TokensLocked events
        let filter = Filter::new()
            .address(gateway_address)
            .event("TokensLocked(bytes32,address,address,uint256,uint64,bytes32,bytes32,uint256)");

        let mut stream = provider.subscribe_logs(&filter).await?;

        info!("Subscribed to gateway events on chain {}", chain_id);

        while let Some(log) = stream.next().await {
            debug!("Received TokensLocked event on chain {}: {:?}", chain_id, log);

            if let Err(e) = Self::handle_tokens_locked(
                chain_id,
                log,
                &coordinator,
                &p2p,
            )
            .await
            {
                warn!("Failed to handle TokensLocked event: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_tokens_locked(
        source_chain_id: u64,
        log: Log,
        coordinator: &CoordinatorClient,
        p2p: &P2PNetwork,
    ) -> Result<()> {
        // Parse event data
        let deposit_id = hex::encode(log.topics[1].as_bytes());
        let sender = format!("0x{}", hex::encode(&log.topics[2].as_bytes()[12..]));
        let token = format!("0x{}", hex::encode(&log.topics[3].as_bytes()[12..]));
        
        // Parse amount, target_chain_id, recipient, zcash_address from log.data
        // Simplified parsing for example
        let amount = u64::from_be_bytes(log.data[0..8].try_into().unwrap());
        let target_chain_id = u64::from_be_bytes(log.data[8..16].try_into().unwrap());
        let recipient = log.data[16..48].to_vec();
        let zcash_address = log.data[48..80].to_vec();

        info!(
            "TokensLocked event: deposit_id={}, source={}, target={}",
            deposit_id, source_chain_id, target_chain_id
        );

        // Notify coordinator (coordinator will create Zcash note)
        let notification = DepositNotification {
            deposit_id: deposit_id.clone(),
            source_chain_id,
            target_chain_id,
            sender,
            token,
            amount,
            recipient,
            zcash_address,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        coordinator.notify_deposit(notification).await?;

        // Broadcast to P2P that we've notified coordinator
        p2p.broadcast_deposit_notification(&deposit_id).await?;

        info!("Notified coordinator about deposit: {}", deposit_id);

        Ok(())
    }
}

// ============ Solana Event Listener ============

struct SolanaEventListener {
    chain_config: crate::config::ChainConfig,
    coordinator_client: Arc<CoordinatorClient>,
    tx_executor: Arc<TransactionExecutor>,
    p2p_network: Arc<P2PNetwork>,
    db: RelayerDatabase,
}

impl SolanaEventListener {
    async fn new(
        chain_config: crate::config::ChainConfig,
        coordinator_client: Arc<CoordinatorClient>,
        tx_executor: Arc<TransactionExecutor>,
        p2p_network: Arc<P2PNetwork>,
        db: RelayerDatabase,
    ) -> Result<Self> {
        Ok(Self {
            chain_config,
            coordinator_client,
            tx_executor,
            p2p_network,
            db,
        })
    }
}

#[async_trait::async_trait]
impl EventListener for SolanaEventListener {
    async fn start(&mut self) -> Result<()> {
        info!(
            "Starting Solana event listener for chain: {}",
            self.chain_config.name
        );

        let chain_id = self.chain_config.chain_id;
        let rpc_url = self.chain_config.rpc_url.clone();
        let coordinator = self.coordinator_client.clone();

        tokio::spawn(async move {
            if let Err(e) = Self::listen_loop(chain_id, &rpc_url, coordinator).await {
                warn!("Solana listener error for chain {}: {}", chain_id, e);
            }
        });

        Ok(())
    }
}

impl SolanaEventListener {
    async fn listen_loop(
        chain_id: u64,
        rpc_url: &str,
        _coordinator: Arc<CoordinatorClient>,
    ) -> Result<()> {
        use solana_client::rpc_client::RpcClient;

        let _client = RpcClient::new(rpc_url.to_string());

        info!("Connected to Solana RPC on chain {}", chain_id);

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            debug!("Polling Solana for new transactions on chain {}", chain_id);
            
            // Poll for TokensLocked events and notify coordinator
            // Similar to EVM implementation
        }
    }
}

// ============ NEAR Event Listener ============

struct NearEventListener {
    chain_config: crate::config::ChainConfig,
    coordinator_client: Arc<CoordinatorClient>,
    tx_executor: Arc<TransactionExecutor>,
    p2p_network: Arc<P2PNetwork>,
    db: RelayerDatabase,
}

impl NearEventListener {
    async fn new(
        chain_config: crate::config::ChainConfig,
        coordinator_client: Arc<CoordinatorClient>,
        tx_executor: Arc<TransactionExecutor>,
        p2p_network: Arc<P2PNetwork>,
        db: RelayerDatabase,
    ) -> Result<Self> {
        Ok(Self {
            chain_config,
            coordinator_client,
            tx_executor,
            p2p_network,
            db,
        })
    }
}

#[async_trait::async_trait]
impl EventListener for NearEventListener {
    async fn start(&mut self) -> Result<()> {
        info!(
            "Starting NEAR event listener for chain: {}",
            self.chain_config.name
        );

        let chain_id = self.chain_config.chain_id;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                debug!("Polling NEAR for events on chain {}", chain_id);
            }
        });

        Ok(())
    }
}

// ============ Mina Event Listener ============

struct MinaEventListener {
    chain_config: crate::config::ChainConfig,
    coordinator_client: Arc<CoordinatorClient>,
    tx_executor: Arc<TransactionExecutor>,
    p2p_network: Arc<P2PNetwork>,
    db: RelayerDatabase,
}

impl MinaEventListener {
    async fn new(
        chain_config: crate::config::ChainConfig,
        coordinator_client: Arc<CoordinatorClient>,
        tx_executor: Arc<TransactionExecutor>,
        p2p_network: Arc<P2PNetwork>,
        db: RelayerDatabase,
    ) -> Result<Self> {
        Ok(Self {
            chain_config,
            coordinator_client,
            tx_executor,
            p2p_network,
            db,
        })
    }
}

#[async_trait::async_trait]
impl EventListener for MinaEventListener {
    async fn start(&mut self) -> Result<()> {
        info!(
            "Starting Mina event listener for chain: {}",
            self.chain_config.name
        );

        let chain_id = self.chain_config.chain_id;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                debug!("Polling Mina for events on chain {}", chain_id);
            }
        });

        Ok(())
    }
}