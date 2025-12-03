// relayer/src/database.rs
//! Relayer-specific database
//! FOCUSED: Track relay execution and earnings only
//! Does NOT duplicate coordinator's deposit/withdrawal tracking

use anyhow::Result;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tracing::info;

#[derive(Clone)]
pub struct RelayerDatabase {
    pool: SqlitePool,
}

/// Withdrawal execution record (what we executed)
#[derive(Debug, Clone)]
pub struct WithdrawalExecution {
    pub withdrawal_id: String,
    pub tx_hash: String,
    pub chain_id: u64,
    pub executed_at: i64,
    pub gas_used: u64,
    pub fee_earned: u64,
}

/// Relayer performance statistics
#[derive(Debug, Default)]
pub struct RelayerStats {
    pub withdrawals_executed: u64,
    pub total_rewards: u64,
    pub successful_relays: u64,
    pub failed_relays: u64,
    pub total_gas_spent: u64,
}

impl RelayerDatabase {
    pub async fn new(path: &str) -> Result<Self> {
        let url = format!("sqlite:{}", path);
        
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await?;
        
        Self::create_tables(&pool).await?;
        
        info!("Relayer database initialized at {}", path);
        
        Ok(Self { pool })
    }

    async fn create_tables(pool: &SqlitePool) -> Result<()> {
        // Track withdrawal executions (what we relayed)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS withdrawal_executions (
                withdrawal_id TEXT PRIMARY KEY,
                tx_hash TEXT NOT NULL,
                chain_id INTEGER NOT NULL,
                executed_at INTEGER NOT NULL,
                gas_used INTEGER NOT NULL,
                fee_earned INTEGER NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        // Track relay performance
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS relay_performance (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                withdrawal_id TEXT NOT NULL,
                success INTEGER NOT NULL,
                error_message TEXT,
                timestamp INTEGER NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        // Track P2P task claims
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS task_claims (
                task_id TEXT PRIMARY KEY,
                claimed_by TEXT NOT NULL,
                claimed_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL
            )",
        )
        .execute(pool)
        .await?;

        // Create indexes
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_executions_chain 
             ON withdrawal_executions(chain_id)"
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_performance_timestamp 
             ON relay_performance(timestamp)"
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Record successful withdrawal execution
    pub async fn record_withdrawal_execution(
        &self,
        withdrawal_id: &str,
        tx_hash: &str,
        executed_at: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO withdrawal_executions 
             (withdrawal_id, tx_hash, chain_id, executed_at, gas_used, fee_earned) 
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(withdrawal_id)
        .bind(tx_hash)
        .bind(0i64) // Chain ID to be filled
        .bind(executed_at)
        .bind(0i64) // Gas used to be filled
        .bind(0i64) // Fee earned to be filled
        .execute(&self.pool)
        .await?;

        // Record successful relay
        self.record_relay_performance(withdrawal_id, true, None).await?;

        Ok(())
    }

    /// Record relay performance
    pub async fn record_relay_performance(
        &self,
        withdrawal_id: &str,
        success: bool,
        error_message: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO relay_performance 
             (withdrawal_id, success, error_message, timestamp) 
             VALUES (?, ?, ?, ?)"
        )
        .bind(withdrawal_id)
        .bind(success as i32)
        .bind(error_message)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Store P2P task claim
    pub async fn store_task_claim(
        &self,
        task_id: &str,
        claimed_by: &str,
        ttl_seconds: i64,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + ttl_seconds;

        sqlx::query(
            "INSERT OR REPLACE INTO task_claims 
             (task_id, claimed_by, claimed_at, expires_at) 
             VALUES (?, ?, ?, ?)"
        )
        .bind(task_id)
        .bind(claimed_by)
        .bind(now)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check if task is claimed by someone else
    pub async fn is_task_claimed(&self, task_id: &str) -> Result<bool> {
        let now = chrono::Utc::now().timestamp();

        let result: Option<(String,)> = sqlx::query_as(
            "SELECT claimed_by FROM task_claims 
             WHERE task_id = ? AND expires_at > ?"
        )
        .bind(task_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.is_some())
    }

    /// Get relayer statistics
    pub async fn get_stats(&self) -> Result<RelayerStats> {
        let executions: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM withdrawal_executions"
        )
        .fetch_one(&self.pool)
        .await?;

        let successful: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM relay_performance WHERE success = 1"
        )
        .fetch_one(&self.pool)
        .await?;

        let failed: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM relay_performance WHERE success = 0"
        )
        .fetch_one(&self.pool)
        .await?;

        let total_gas: (Option<i64>,) = sqlx::query_as(
            "SELECT SUM(gas_used) FROM withdrawal_executions"
        )
        .fetch_one(&self.pool)
        .await?;

        let total_rewards: (Option<i64>,) = sqlx::query_as(
            "SELECT SUM(fee_earned) FROM withdrawal_executions"
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(RelayerStats {
            withdrawals_executed: executions.0 as u64,
            total_rewards: total_rewards.0.unwrap_or(0) as u64,
            successful_relays: successful.0 as u64,
            failed_relays: failed.0 as u64,
            total_gas_spent: total_gas.0.unwrap_or(0) as u64,
        })
    }

    /// Get execution history for a specific chain
    pub async fn get_executions_for_chain(&self, chain_id: u64) -> Result<Vec<WithdrawalExecution>> {
        let rows = sqlx::query_as::<_, (String, String, i64, i64, i64, i64)>(
            "SELECT withdrawal_id, tx_hash, chain_id, executed_at, gas_used, fee_earned 
             FROM withdrawal_executions 
             WHERE chain_id = ? 
             ORDER BY executed_at DESC 
             LIMIT 100"
        )
        .bind(chain_id as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| WithdrawalExecution {
            withdrawal_id: r.0,
            tx_hash: r.1,
            chain_id: r.2 as u64,
            executed_at: r.3,
            gas_used: r.4 as u64,
            fee_earned: r.5 as u64,
        }).collect())
    }

    /// Clean up expired task claims
    pub async fn cleanup_expired_claims(&self) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "DELETE FROM task_claims WHERE expires_at < ?"
        )
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}