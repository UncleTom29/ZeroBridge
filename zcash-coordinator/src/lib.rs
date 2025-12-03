// zcash-coordinator/src/lib.rs
//! ZeroBridge Zcash Coordinator Library
//! 
//! This library provides the core functionality for the ZeroBridge coordinator,
//! which orchestrates privacy-preserving cross-chain transfers using Zcash's
//! shielded transaction technology.

pub mod config;
pub mod shielded_pool;
pub mod token_registry;
pub mod liquidity_manager;
pub mod database;
pub mod rpc_server;
pub mod zcash_client;

// Re-export commonly used types
pub use config::{Config, ZcashConfig, ChainConfig};
pub use shielded_pool::ShieldedPoolManager;
pub use token_registry::TokenRegistry;
pub use liquidity_manager::LiquidityManager;
pub use database::Database;
pub use zcash_client::ZcashClient;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}