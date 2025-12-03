// zcash-coordinator/src/liquidity_manager.rs
//! Liquidity management across chains

use anyhow::{Context, Result};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::config::LiquidityConfig;
use crate::database::Database;

/// Manages liquidity pools across all gateway chains
pub struct LiquidityManager {
    db: Database,
    config: LiquidityConfig,
    pools: HashMap<PoolKey, LiquidityPool>,
}

/// Pool identifier (chain_id, token_address)
type PoolKey = (u64, String);

/// Liquidity pool state
#[derive(Debug, Clone)]
pub struct LiquidityPool {
    pub chain_id: u64,
    pub token: String,
    pub available: u64,
    pub locked: u64,
    pub target: u64,
    pub last_rebalance: u64,
}

impl LiquidityPool {
    /// Calculate utilization ratio
    pub fn utilization(&self) -> f64 {
        let total = self.available + self.locked;
        if total == 0 {
            return 0.0;
        }
        self.locked as f64 / total as f64
    }
    
    /// Check if rebalancing is needed
    pub fn needs_rebalancing(&self, threshold: f64) -> bool {
        let utilization = self.utilization();
        utilization > threshold
    }
    
    /// Calculate rebalance amount
    pub fn calculate_rebalance_amount(&self, target_utilization: f64) -> i64 {
        let total = self.available + self.locked;
        let current_locked = self.locked as f64;
        let target_locked = total as f64 * target_utilization;
        
        (target_locked - current_locked) as i64
    }
}

impl LiquidityManager {
    /// Create new liquidity manager
    pub async fn new(
        db: Database,
        config: LiquidityConfig,
    ) -> Result<Self> {
        let mut manager = Self {
            db,
            config,
            pools: HashMap::new(),
        };
        
        // Load existing pool states from database
        manager.load_pools().await?;
        
        Ok(manager)
    }
    
    /// Ensure sufficient liquidity for a withdrawal
    pub async fn ensure_liquidity(
        &self,
        chain_id: u64,
        token: &str,
        amount: u64,
    ) -> Result<()> {
        debug!(
            "Checking liquidity: chain={}, token={}, amount={}",
            chain_id, token, amount
        );
        
        let key = (chain_id, token.to_string());
        let pool = self.pools.get(&key)
            .context("Pool not found")?;
        
        if pool.available < amount {
            anyhow::bail!(
                "Insufficient liquidity: need {}, have {}",
                amount,
                pool.available
            );
        }
        
        Ok(())
    }
    
    /// Lock liquidity for a pending withdrawal
    pub async fn lock_liquidity(
        &mut self,
        chain_id: u64,
        token: &str,
        amount: u64,
    ) -> Result<()> {
        let key = (chain_id, token.to_string());
        let pool = self.pools.get_mut(&key)
            .context("Pool not found")?;
        
        if pool.available < amount {
            anyhow::bail!("Insufficient available liquidity");
        }
        
        pool.available -= amount;
        pool.locked += amount;
        
        // Update database
        self.db
            .update_liquidity_pool(chain_id, token, pool.available, pool.locked)
            .await?;
        
        debug!("Locked {} liquidity on chain {}", amount, chain_id);
        Ok(())
    }
    
    /// Release locked liquidity after withdrawal completion
    pub async fn release_liquidity(
        &mut self,
        chain_id: u64,
        token: &str,
        amount: u64,
    ) -> Result<()> {
        let key = (chain_id, token.to_string());
        let pool = self.pools.get_mut(&key)
            .context("Pool not found")?;
        
        if pool.locked < amount {
            warn!("Attempting to release more than locked: {}", amount);
            return Ok(());
        }
        
        pool.locked -= amount;
        
        // Update database
        self.db
            .update_liquidity_pool(chain_id, token, pool.available, pool.locked)
            .await?;
        
        debug!("Released {} liquidity on chain {}", amount, chain_id);
        Ok(())
    }
    
    /// Add liquidity to a pool
    pub async fn add_liquidity(
        &mut self,
        chain_id: u64,
        token: &str,
        amount: u64,
    ) -> Result<()> {
        let key = (chain_id, token.to_string());
        
        let pool = self.pools.entry(key.clone()).or_insert(LiquidityPool {
            chain_id,
            token: token.to_string(),
            available: 0,
            locked: 0,
            target: 0,
            last_rebalance: 0,
        });
        
        pool.available += amount;
        
        // Update database
        self.db
            .update_liquidity_pool(chain_id, token, pool.available, pool.locked)
            .await?;
        
        info!("Added {} liquidity to chain {}", amount, chain_id);
        Ok(())
    }
    
    /// Remove liquidity from a pool
    pub async fn remove_liquidity(
        &mut self,
        chain_id: u64,
        token: &str,
        amount: u64,
    ) -> Result<()> {
        let key = (chain_id, token.to_string());
        let pool = self.pools.get_mut(&key)
            .context("Pool not found")?;
        
        if pool.available < amount {
            anyhow::bail!("Insufficient available liquidity");
        }
        
        pool.available -= amount;
        
        // Update database
        self.db
            .update_liquidity_pool(chain_id, token, pool.available, pool.locked)
            .await?;
        
        info!("Removed {} liquidity from chain {}", amount, chain_id);
        Ok(())
    }
    
    /// Check which pools need rebalancing
    pub async fn check_rebalancing_needed(&self) -> Result<Vec<(u64, String)>> {
        let mut needs_rebalancing = Vec::new();
        
        for (key, pool) in &self.pools {
            if pool.needs_rebalancing(self.config.rebalance_threshold) {
                info!(
                    "Pool needs rebalancing: chain={}, token={}, utilization={:.2}%",
                    pool.chain_id,
                    pool.token,
                    pool.utilization() * 100.0
                );
                needs_rebalancing.push(key.clone());
            }
        }
        
        Ok(needs_rebalancing)
    }
    
    /// Trigger rebalancing for a specific pool
    pub async fn trigger_rebalance(
        &mut self,
        chain_id: u64,
        token: &str,
    ) -> Result<()> {
        info!("Triggering rebalance for chain {} token {}", chain_id, token);
        
        let key = (chain_id, token.to_string());
        let pool = self.pools.get_mut(&key)
            .context("Pool not found")?;
        
        // Calculate rebalance amount
        let amount = pool.calculate_rebalance_amount(self.config.target_utilization);
        
        if amount.abs() as u64 > self.config.max_rebalance_usd {
            warn!(
                "Rebalance amount {} exceeds maximum {}",
                amount.abs(),
                self.config.max_rebalance_usd
            );
            return Ok(());
        }
        
        if amount > 0 {
            // Need to add liquidity
            info!("Need to add {} liquidity to chain {}", amount, chain_id);
            // In production, this would trigger cross-chain transfer
            
        } else if amount < 0 {
            // Need to remove liquidity
            info!("Need to remove {} liquidity from chain {}", amount.abs(), chain_id);
            // In production, this would trigger cross-chain transfer
        }
        
        pool.last_rebalance = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Ok(())
    }
    
    /// Get pool state
    pub fn get_pool(&self, chain_id: u64, token: &str) -> Option<&LiquidityPool> {
        let key = (chain_id, token.to_string());
        self.pools.get(&key)
    }
    
    /// Get all pools
    pub fn get_all_pools(&self) -> Vec<&LiquidityPool> {
        self.pools.values().collect()
    }
    
    /// Load pool states from database
    async fn load_pools(&mut self) -> Result<()> {
        let pools = self.db.get_all_liquidity_pools().await?;
        
        for (chain_id, token, available, locked, target) in pools {
            let key = (chain_id, token.clone());
            self.pools.insert(
                key,
                LiquidityPool {
                    chain_id,
                    token,
                    available,
                    locked,
                    target,
                    last_rebalance: 0,
                },
            );
        }
        
        info!("Loaded {} liquidity pools from database", self.pools.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_utilization() {
        let pool = LiquidityPool {
            chain_id: 1,
            token: "ETH".to_string(),
            available: 100,
            locked: 50,
            target: 200,
            last_rebalance: 0,
        };
        
        assert_eq!(pool.utilization(), 0.3333333333333333);
    }
    
    #[test]
    fn test_needs_rebalancing() {
        let pool = LiquidityPool {
            chain_id: 1,
            token: "ETH".to_string(),
            available: 20,
            locked: 80,
            target: 200,
            last_rebalance: 0,
        };
        
        assert!(pool.needs_rebalancing(0.7)); // 80% > 70%
        assert!(!pool.needs_rebalancing(0.9)); // 80% < 90%
    }
    
    #[test]
    fn test_calculate_rebalance_amount() {
        let pool = LiquidityPool {
            chain_id: 1,
            token: "ETH".to_string(),
            available: 100,
            locked: 0,
            target: 200,
            last_rebalance: 0,
        };
        
        // Target 50% utilization: need to lock 50
        let amount = pool.calculate_rebalance_amount(0.5);
        assert_eq!(amount, 50);
    }
}