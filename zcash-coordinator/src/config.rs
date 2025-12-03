// zcash-coordinator/src/config.rs
//! Configuration structures for the coordinator

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Zcash node configuration
    pub zcash: ZcashConfig,
    
    /// Gateway chains configuration
    pub chains: Vec<ChainConfig>,
    
    /// Token registry file path
    pub tokens_config: String,
    
    /// Liquidity management configuration
    pub liquidity: LiquidityConfig,
    
    /// Polling interval in seconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZcashConfig {
    /// Network type (mainnet, testnet, regtest)
    pub network: ZcashNetwork,
    
    /// RPC URL
    pub rpc_url: String,
    
    /// RPC username
    pub rpc_user: String,
    
    /// RPC password
    pub rpc_password: String,
    
    /// Spending key (base58-encoded)
    pub spending_key: String,
    
    /// Number of confirmations required
    #[serde(default = "default_confirmations")]
    pub confirmations: u32,
    
    /// Enable Orchard (default: true)
    #[serde(default = "default_true")]
    pub enable_orchard: bool,
    
    /// Enable Sapling (default: true)
    #[serde(default = "default_true")]
    pub enable_sapling: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ZcashNetwork {
    Mainnet,
    Testnet,
    Regtest,
}

impl ZcashNetwork {
    pub fn is_mainnet(&self) -> bool {
        matches!(self, ZcashNetwork::Mainnet)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    /// Chain ID
    pub chain_id: u64,
    
    /// Chain name
    pub name: String,
    
    /// Chain type
    pub chain_type: ChainType,
    
    /// RPC URL
    pub rpc_url: String,
    
    /// WebSocket URL (optional)
    pub ws_url: Option<String>,
    
    /// Gateway contract address
    pub gateway_address: String,
    
    /// Start block for event scanning
    pub start_block: u64,
    
    /// Enabled flag
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    /// Required confirmations
    #[serde(default = "default_confirmations")]
    pub confirmations: u32,
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
    Starknet,
    Osmosis,
}

impl ChainType {
    pub fn is_evm(&self) -> bool {
        matches!(
            self,
            ChainType::Ethereum | ChainType::Base | ChainType::Polygon
        )
    }
    
    pub fn is_non_evm(&self) -> bool {
        !self.is_evm()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityConfig {
    /// Rebalance threshold (0.0 - 1.0)
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold: f64,
    
    /// Target utilization ratio (0.0 - 1.0)
    #[serde(default = "default_target_utilization")]
    pub target_utilization: f64,
    
    /// Minimum liquidity per chain (in USD equivalent)
    #[serde(default = "default_min_liquidity")]
    pub min_liquidity_usd: u64,
    
    /// Maximum single rebalance amount (in USD equivalent)
    #[serde(default = "default_max_rebalance")]
    pub max_rebalance_usd: u64,
}

// Default values
fn default_poll_interval() -> u64 {
    10 // 10 seconds
}

fn default_confirmations() -> u32 {
    6
}

fn default_true() -> bool {
    true
}

fn default_rebalance_threshold() -> f64 {
    0.8 // 80%
}

fn default_target_utilization() -> f64 {
    0.5 // 50%
}

fn default_min_liquidity() -> u64 {
    10_000 // $10k
}

fn default_max_rebalance() -> u64 {
    100_000 // $100k
}

impl Config {
    /// Load configuration from TOML file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read config file")?;
        
        let config: Config = toml::from_str(&content)
            .context("Failed to parse config file")?;
        
        config.validate()?;
        
        Ok(config)
    }
    
    /// Validate configuration
    fn validate(&self) -> Result<()> {
        // Validate Zcash config
        if self.zcash.rpc_url.is_empty() {
            anyhow::bail!("Zcash RPC URL cannot be empty");
        }
        
        if self.zcash.spending_key.is_empty() {
            anyhow::bail!("Zcash spending key cannot be empty");
        }
        
        // Validate chains
        if self.chains.is_empty() {
            anyhow::bail!("At least one chain must be configured");
        }
        
        for chain in &self.chains {
            if chain.rpc_url.is_empty() {
                anyhow::bail!("RPC URL for chain {} cannot be empty", chain.name);
            }
            
            if chain.gateway_address.is_empty() {
                anyhow::bail!("Gateway address for chain {} cannot be empty", chain.name);
            }
        }
        
        // Validate liquidity config
        if self.liquidity.rebalance_threshold <= 0.0 
            || self.liquidity.rebalance_threshold > 1.0 
        {
            anyhow::bail!("Rebalance threshold must be between 0.0 and 1.0");
        }
        
        if self.liquidity.target_utilization <= 0.0 
            || self.liquidity.target_utilization > 1.0 
        {
            anyhow::bail!("Target utilization must be between 0.0 and 1.0");
        }
        
        Ok(())
    }
    
    /// Get chain config by ID
    pub fn get_chain(&self, chain_id: u64) -> Option<&ChainConfig> {
        self.chains.iter().find(|c| c.chain_id == chain_id)
    }
    
    /// Get enabled chains
    pub fn enabled_chains(&self) -> Vec<&ChainConfig> {
        self.chains.iter().filter(|c| c.enabled).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = Config {
            zcash: ZcashConfig {
                network: ZcashNetwork::Testnet,
                rpc_url: "http://localhost:18232".to_string(),
                rpc_user: "user".to_string(),
                rpc_password: "pass".to_string(),
                spending_key: "test_key".to_string(),
                confirmations: 6,
                enable_orchard: true,
                enable_sapling: true,
            },
            chains: vec![
                ChainConfig {
                    chain_id: 1,
                    name: "Ethereum".to_string(),
                    chain_type: ChainType::Ethereum,
                    rpc_url: "http://localhost:8545".to_string(),
                    ws_url: None,
                    gateway_address: "0x1234".to_string(),
                    start_block: 0,
                    enabled: true,
                    confirmations: 12,
                },
            ],
            tokens_config: "tokens.toml".to_string(),
            liquidity: LiquidityConfig {
                rebalance_threshold: 0.8,
                target_utilization: 0.5,
                min_liquidity_usd: 10_000,
                max_rebalance_usd: 100_000,
            },
            poll_interval: 10,
        };
        
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_invalid_threshold() {
        let mut config = Config {
            zcash: ZcashConfig {
                network: ZcashNetwork::Testnet,
                rpc_url: "http://localhost:18232".to_string(),
                rpc_user: "user".to_string(),
                rpc_password: "pass".to_string(),
                spending_key: "test_key".to_string(),
                confirmations: 6,
                enable_orchard: true,
                enable_sapling: true,
            },
            chains: vec![],
            tokens_config: "tokens.toml".to_string(),
            liquidity: LiquidityConfig {
                rebalance_threshold: 1.5, // Invalid
                target_utilization: 0.5,
                min_liquidity_usd: 10_000,
                max_rebalance_usd: 100_000,
            },
            poll_interval: 10,
        };
        
        config.chains.push(ChainConfig {
            chain_id: 1,
            name: "Test".to_string(),
            chain_type: ChainType::Ethereum,
            rpc_url: "http://localhost:8545".to_string(),
            ws_url: None,
            gateway_address: "0x1234".to_string(),
            start_block: 0,
            enabled: true,
            confirmations: 12,
        });
        
        assert!(config.validate().is_err());
    }
}