// ============================================
// relayer/src/stake_manager.rs
//! Manage relayer stake

use anyhow::Result;
use tracing::info;

use crate::config::RelayerConfig;
use crate::database::RelayerDatabase;

pub struct StakeManager {
    config: RelayerConfig,
    _db: RelayerDatabase,
}

impl StakeManager {
    pub async fn new(config: RelayerConfig, db: RelayerDatabase) -> Result<Self> {
        Ok(Self { config, _db: db })
    }

    pub async fn ensure_minimum_stake(&self) -> Result<()> {
        if self.config.staking.current_stake < self.config.staking.minimum_stake {
            anyhow::bail!("Stake below minimum");
        }
        Ok(())
    }

    pub async fn get_pending_rewards(&self) -> Result<u64> {
        // Query hub contract for rewards
        Ok(0)
    }

    pub async fn claim_rewards(&self) -> Result<()> {
        info!("Claiming rewards");
        Ok(())
    }
}
