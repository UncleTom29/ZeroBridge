// relayer/src/config.rs
//! Relayer configuration - focused on relay-specific settings
//! NO overlap with coordinator config

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayerConfig {
    /// Coordinator RPC URL (read-only access)
    pub coordinator_url: String,
    
    /// Chains to relay for
    pub chains: Vec<ChainConfig>,
    
    /// Relayer identity
    pub relayer_identity: RelayerIdentity,
    
    /// Staking configuration
    pub staking: StakingConfig,
    
    /// P2P network configuration
    pub p2p: P2PConfig,
    
    /// Database path
    pub database_path: String,
    
    /// Polling interval in seconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u64,
    
    /// Maximum concurrent relay tasks
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_tasks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub name: String,
    pub chain_type: ChainType,
    
    /// RPC endpoint for transaction submission
    pub rpc_url: String,
    
    /// WebSocket for event listening
    pub ws_url: Option<String>,
    
    /// Gateway contract address
    pub gateway_address: String,
    
    /// Private key for transaction signing
    pub private_key: String,
    
    /// Gas price strategy
    pub gas_strategy: GasStrategy,
    
    /// Transaction retry settings
    pub retry_config: RetryConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChainType {
    Ethereum,
    Base,
    Polygon,
    Solana,
    Near,
    Mina,
}

impl ChainType {
    pub fn is_evm(&self) -> bool {
        matches!(self, ChainType::Ethereum | ChainType::Base | ChainType::Polygon)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayerIdentity {
    /// Relayer public address/ID
    pub address: String,
    
    /// Relayer name (for P2P identification)
    pub name: String,
    
    /// Reputation score (tracked by network)
    #[serde(default)]
    pub reputation: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingConfig {
    /// Minimum stake required to be active relayer
    pub minimum_stake: u64,
    
    /// Current staked amount
    pub current_stake: u64,
    
    /// Hub contract address for staking
    pub hub_contract: String,
    
    /// Chain ID where hub is deployed
    pub hub_chain_id: u64,
    
    /// Auto-restake rewards
    #[serde(default = "default_true")]
    pub auto_restake: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PConfig {
    /// Listen address for P2P
    #[serde(default = "default_p2p_listen")]
    pub listen_addr: String,
    
    /// P2P port
    #[serde(default = "default_p2p_port")]
    pub port: u16,
    
    /// Bootstrap peers
    pub bootstrap_peers: Vec<String>,
    
    /// Maximum peer connections
    #[serde(default = "default_max_peers")]
    pub max_peers: usize,
    
    /// Gossip protocol settings
    pub gossip: GossipConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipConfig {
    /// Heartbeat interval in seconds
    #[serde(default = "default_heartbeat")]
    pub heartbeat_interval: u64,
    
    /// Message TTL
    #[serde(default = "default_message_ttl")]
    pub message_ttl: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasStrategy {
    /// Strategy type
    pub strategy_type: GasStrategyType,
    
    /// Max gas price (in gwei for EVM)
    pub max_gas_price: u64,
    
    /// Gas price multiplier
    #[serde(default = "default_gas_multiplier")]
    pub multiplier: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GasStrategyType {
    Fast,
    Standard,
    Slow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    
    /// Initial backoff in seconds
    #[serde(default = "default_initial_backoff")]
    pub initial_backoff: u64,
    
    /// Max backoff in seconds
    #[serde(default = "default_max_backoff")]
    pub max_backoff: u64,
}

// Default values
fn default_poll_interval() -> u64 {
    5
}

fn default_max_concurrent() -> usize {
    10
}

fn default_true() -> bool {
    true
}

fn default_p2p_listen() -> String {
    "0.0.0.0".to_string()
}

fn default_p2p_port() -> u16 {
    9000
}

fn default_max_peers() -> usize {
    50
}

fn default_heartbeat() -> u64 {
    30
}

fn default_message_ttl() -> u64 {
    300
}

fn default_gas_multiplier() -> f64 {
    1.2
}

fn default_max_retries() -> u32 {
    3
}

fn default_initial_backoff() -> u64 {
    5
}

fn default_max_backoff() -> u64 {
    300
}

impl RelayerConfig {
    /// Load configuration from file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read config file")?;
        
        let config: RelayerConfig = toml::from_str(&content)
            .context("Failed to parse config file")?;
        
        config.validate()?;
        
        Ok(config)
    }
    
    /// Validate configuration
    fn validate(&self) -> Result<()> {
        // Validate coordinator URL
        if self.coordinator_url.is_empty() {
            anyhow::bail!("Coordinator URL cannot be empty");
        }
        
        // Validate chains
        if self.chains.is_empty() {
            anyhow::bail!("At least one chain must be configured");
        }
        
        for chain in &self.chains {
            if chain.rpc_url.is_empty() {
                anyhow::bail!("RPC URL for chain {} cannot be empty", chain.name);
            }
            
            if chain.private_key.is_empty() {
                anyhow::bail!("Private key for chain {} cannot be empty", chain.name);
            }
        }
        
        // Validate staking
        if self.staking.minimum_stake == 0 {
            anyhow::bail!("Minimum stake must be greater than 0");
        }
        
        // Validate P2P
        if self.p2p.port == 0 {
            anyhow::bail!("P2P port must be greater than 0");
        }
        
        Ok(())
    }
    
    /// Get chain config by ID
    pub fn get_chain(&self, chain_id: u64) -> Option<&ChainConfig> {
        self.chains.iter().find(|c| c.chain_id == chain_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = RelayerConfig {
            coordinator_url: "http://localhost:8080".to_string(),
            chains: vec![ChainConfig {
                chain_id: 1,
                name: "Ethereum".to_string(),
                chain_type: ChainType::Ethereum,
                rpc_url: "http://localhost:8545".to_string(),
                ws_url: None,
                gateway_address: "0x123".to_string(),
                private_key: "0xabc".to_string(),
                gas_strategy: GasStrategy {
                    strategy_type: GasStrategyType::Standard,
                    max_gas_price: 100,
                    multiplier: 1.2,
                },
                retry_config: RetryConfig {
                    max_retries: 3,
                    initial_backoff: 5,
                    max_backoff: 300,
                },
            }],
            relayer_identity: RelayerIdentity {
                address: "0x456".to_string(),
                name: "test-relayer".to_string(),
                reputation: 100,
            },
            staking: StakingConfig {
                minimum_stake: 100,
                current_stake: 150,
                hub_contract: "0x789".to_string(),
                hub_chain_id: 1,
                auto_restake: true,
            },
            p2p: P2PConfig {
                listen_addr: "0.0.0.0".to_string(),
                port: 9000,
                bootstrap_peers: vec![],
                max_peers: 50,
                gossip: GossipConfig {
                    heartbeat_interval: 30,
                    message_ttl: 300,
                },
            },
            database_path: "relayer.db".to_string(),
            poll_interval: 5,
            max_concurrent_tasks: 10,
        };
        
        assert!(config.validate().is_ok());
    }
}