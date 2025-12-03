// zcash-coordinator/src/database.rs
//! SQLite database for coordinator state persistence
//! FOCUSED: Track deposit/withdrawal state and authorization

use anyhow::Result;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::path::Path;
use tracing::info;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

/// Deposit record
#[derive(Debug, Clone)]
pub struct Deposit {
    pub deposit_id: String,
    pub source_chain_id: u64,
    pub target_chain_id: u64,
    pub sender: String,
    pub recipient: Vec<u8>,
    pub token: String,
    pub amount: u64,
    pub zcash_address: Vec<u8>,
    pub processed: bool,
    pub zcash_txid: Option<String>,
    pub note_commitment: Option<String>,
    pub created_at: i64,
}

/// Withdrawal record
#[derive(Debug, Clone)]
pub struct Withdrawal {
    pub withdrawal_id: String,
    pub target_chain_id: u64,
    pub recipient: String,
    pub token: String,
    pub amount: u64,
    pub nullifier: Vec<u8>,
    pub zcash_proof: Vec<u8>,
    pub merkle_root: Vec<u8>,
    pub authorized: bool,
    pub auth_signature: Option<Vec<u8>>,
    pub created_at: i64,
}

/// Statistics
#[derive(Debug, Default)]
pub struct Stats {
    pub total_deposits: u64,
    pub total_withdrawals: u64,
    pub total_volume: u64,
    pub active_deposits: u64,
}

impl Database {
    /// Create new database connection
    pub async fn new(path: &Path) -> Result<Self> {
        let url = format!("sqlite:{}", path.display());
        
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(&url)
            .await?;
        
        // Create tables
        Self::create_tables(&pool).await?;
        
        info!("Database initialized at {:?}", path);
        
        Ok(Self { pool })
    }
    
    /// Create database tables
    async fn create_tables(pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS deposits (
                deposit_id TEXT PRIMARY KEY,
                source_chain_id INTEGER NOT NULL,
                target_chain_id INTEGER NOT NULL,
                sender TEXT NOT NULL,
                recipient BLOB NOT NULL,
                token TEXT NOT NULL,
                amount INTEGER NOT NULL,
                zcash_address BLOB NOT NULL,
                processed INTEGER NOT NULL DEFAULT 0,
                zcash_txid TEXT,
                note_commitment TEXT,
                created_at INTEGER NOT NULL
            )"
        )
        .execute(pool)
        .await?;
        
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS withdrawals (
                withdrawal_id TEXT PRIMARY KEY,
                target_chain_id INTEGER NOT NULL,
                recipient TEXT NOT NULL,
                token TEXT NOT NULL,
                amount INTEGER NOT NULL,
                nullifier BLOB NOT NULL,
                zcash_proof BLOB NOT NULL,
                merkle_root BLOB NOT NULL,
                authorized INTEGER NOT NULL DEFAULT 0,
                auth_signature BLOB,
                created_at INTEGER NOT NULL
            )"
        )
        .execute(pool)
        .await?;
        
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS nullifiers (
                nullifier TEXT PRIMARY KEY,
                spent INTEGER NOT NULL DEFAULT 0,
                withdrawal_id TEXT,
                spent_at INTEGER
            )"
        )
        .execute(pool)
        .await?;
        
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS shielded_notes (
                commitment TEXT PRIMARY KEY,
                txid TEXT NOT NULL,
                amount INTEGER NOT NULL,
                source_chain_id INTEGER NOT NULL,
                token TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )"
        )
        .execute(pool)
        .await?;
        
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS liquidity_pools (
                chain_id INTEGER NOT NULL,
                token TEXT NOT NULL,
                available INTEGER NOT NULL,
                locked INTEGER NOT NULL,
                target INTEGER NOT NULL,
                PRIMARY KEY (chain_id, token)
            )"
        )
        .execute(pool)
        .await?;
        
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS zcash_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                block_height INTEGER NOT NULL,
                best_block_hash TEXT NOT NULL,
                sync_progress REAL NOT NULL,
                updated_at INTEGER NOT NULL
            )"
        )
        .execute(pool)
        .await?;
        
        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_deposits_processed ON deposits(processed)")
            .execute(pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_withdrawals_authorized ON withdrawals(authorized)")
            .execute(pool).await?;
        
        Ok(())
    }
    
    // ============ Deposit Operations ============
    
    pub async fn store_deposit(&self, deposit: &Deposit) -> Result<()> {
        sqlx::query(
            "INSERT INTO deposits VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&deposit.deposit_id)
        .bind(deposit.source_chain_id as i64)
        .bind(deposit.target_chain_id as i64)
        .bind(&deposit.sender)
        .bind(&deposit.recipient)
        .bind(&deposit.token)
        .bind(deposit.amount as i64)
        .bind(&deposit.zcash_address)
        .bind(deposit.processed as i32)
        .bind(&deposit.zcash_txid)
        .bind(&deposit.note_commitment)
        .bind(deposit.created_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn get_pending_deposits(&self) -> Result<Vec<Deposit>> {
        let rows = sqlx::query_as::<_, (String, i64, i64, String, Vec<u8>, String, i64, Vec<u8>, i32, Option<String>, Option<String>, i64)>(
            "SELECT * FROM deposits WHERE processed = 0 ORDER BY created_at ASC"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|r| Deposit {
            deposit_id: r.0,
            source_chain_id: r.1 as u64,
            target_chain_id: r.2 as u64,
            sender: r.3,
            recipient: r.4,
            token: r.5,
            amount: r.6 as u64,
            zcash_address: r.7,
            processed: r.8 != 0,
            zcash_txid: r.9,
            note_commitment: r.10,
            created_at: r.11,
        }).collect())
    }
    
    pub async fn mark_deposit_processed(
        &self,
        deposit_id: &str,
        note_commitment: &str,
        zcash_txid: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE deposits SET processed = 1, note_commitment = ?, zcash_txid = ? WHERE deposit_id = ?"
        )
        .bind(note_commitment)
        .bind(zcash_txid)
        .bind(deposit_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    // ============ Withdrawal Operations ============
    
    pub async fn store_withdrawal(&self, withdrawal: &Withdrawal) -> Result<()> {
        sqlx::query(
            "INSERT INTO withdrawals VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&withdrawal.withdrawal_id)
        .bind(withdrawal.target_chain_id as i64)
        .bind(&withdrawal.recipient)
        .bind(&withdrawal.token)
        .bind(withdrawal.amount as i64)
        .bind(&withdrawal.nullifier)
        .bind(&withdrawal.zcash_proof)
        .bind(&withdrawal.merkle_root)
        .bind(withdrawal.authorized as i32)
        .bind(&withdrawal.auth_signature)
        .bind(withdrawal.created_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn get_pending_withdrawals(&self) -> Result<Vec<Withdrawal>> {
        let rows = sqlx::query_as::<_, (String, i64, String, String, i64, Vec<u8>, Vec<u8>, Vec<u8>, i32, Option<Vec<u8>>, i64)>(
            "SELECT * FROM withdrawals WHERE authorized = 0 ORDER BY created_at ASC"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|r| Withdrawal {
            withdrawal_id: r.0,
            target_chain_id: r.1 as u64,
            recipient: r.2,
            token: r.3,
            amount: r.4 as u64,
            nullifier: r.5,
            zcash_proof: r.6,
            merkle_root: r.7,
            authorized: r.8 != 0,
            auth_signature: r.9,
            created_at: r.10,
        }).collect())
    }
    
    pub async fn get_authorized_withdrawals(&self) -> Result<Vec<Withdrawal>> {
        let rows = sqlx::query_as::<_, (String, i64, String, String, i64, Vec<u8>, Vec<u8>, Vec<u8>, i32, Option<Vec<u8>>, i64)>(
            "SELECT * FROM withdrawals WHERE authorized = 1 ORDER BY created_at ASC"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|r| Withdrawal {
            withdrawal_id: r.0,
            target_chain_id: r.1 as u64,
            recipient: r.2,
            token: r.3,
            amount: r.4 as u64,
            nullifier: r.5,
            zcash_proof: r.6,
            merkle_root: r.7,
            authorized: r.8 != 0,
            auth_signature: r.9,
            created_at: r.10,
        }).collect())
    }
    
    pub async fn authorize_withdrawal(
        &self,
        withdrawal_id: &str,
        _token_address: &str,
        _amount: u64,
        auth_signature: &[u8],
    ) -> Result<()> {
        sqlx::query(
            "UPDATE withdrawals SET authorized = 1, auth_signature = ? WHERE withdrawal_id = ?"
        )
        .bind(auth_signature)
        .bind(withdrawal_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn mark_withdrawal_invalid(
        &self,
        withdrawal_id: &str,
        _reason: &str,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM withdrawals WHERE withdrawal_id = ?"
        )
        .bind(withdrawal_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    // ============ Nullifier Operations ============
    
    pub async fn mark_nullifier_spent(&self, nullifier: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        sqlx::query(
            "INSERT OR REPLACE INTO nullifiers (nullifier, spent, spent_at) VALUES (?, 1, ?)"
        )
        .bind(nullifier)
        .bind(now)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn is_nullifier_spent(&self, nullifier: &str) -> Result<bool> {
        let result: Option<(i32,)> = sqlx::query_as(
            "SELECT spent FROM nullifiers WHERE nullifier = ?"
        )
        .bind(nullifier)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result.map(|r| r.0 != 0).unwrap_or(false))
    }
    
    // ============ Shielded Note Operations ============
    
    pub async fn store_shielded_note(
        &self,
        commitment: &str,
        txid: &str,
        amount: u64,
        source_chain_id: u64,
        token: &str,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        sqlx::query(
            "INSERT INTO shielded_notes VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(commitment)
        .bind(txid)
        .bind(amount as i64)
        .bind(source_chain_id as i64)
        .bind(token)
        .bind(now)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    // ============ Liquidity Pool Operations ============
    
    pub async fn update_liquidity_pool(
        &self,
        chain_id: u64,
        token: &str,
        available: u64,
        locked: u64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO liquidity_pools (chain_id, token, available, locked, target) 
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(chain_id as i64)
        .bind(token)
        .bind(available as i64)
        .bind(locked as i64)
        .bind(0i64)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn get_all_liquidity_pools(&self) -> Result<Vec<(u64, String, u64, u64, u64)>> {
        let rows = sqlx::query_as::<_, (i64, String, i64, i64, i64)>(
            "SELECT chain_id, token, available, locked, target FROM liquidity_pools"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|r| {
            (r.0 as u64, r.1, r.2 as u64, r.3 as u64, r.4 as u64)
        }).collect())
    }
    
    // ============ Zcash State Operations ============
    
    pub async fn update_zcash_state(
        &self,
        block_height: u32,
        best_block_hash: &str,
        sync_progress: f64,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        sqlx::query(
            "INSERT OR REPLACE INTO zcash_state (id, block_height, best_block_hash, sync_progress, updated_at) 
             VALUES (1, ?, ?, ?, ?)"
        )
        .bind(block_height as i64)
        .bind(best_block_hash)
        .bind(sync_progress)
        .bind(now)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    // ============ Statistics ============
    
    pub async fn get_stats(&self) -> Result<Stats> {
        let deposits: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM deposits WHERE processed = 1"
        )
        .fetch_one(&self.pool)
        .await?;
        
        let withdrawals: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM withdrawals WHERE authorized = 1"
        )
        .fetch_one(&self.pool)
        .await?;
        
        let volume: (Option<i64>,) = sqlx::query_as(
            "SELECT SUM(amount) FROM deposits WHERE processed = 1"
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(Stats {
            total_deposits: deposits.0 as u64,
            total_withdrawals: withdrawals.0 as u64,
            total_volume: volume.0.unwrap_or(0) as u64,
            active_deposits: (deposits.0 - withdrawals.0) as u64,
        })
    }
}