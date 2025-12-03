// zcash-coordinator/src/rpc_server.rs
//! RPC server for coordinator API
//! Relayers communicate with coordinator via this API

use axum::{
    extract::Path,
    routing::{get, post},
    Router,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::database::{Database, Deposit, Withdrawal};
use crate::shielded_pool::ShieldedPoolManager;
use crate::token_registry::TokenRegistry;
use crate::liquidity_manager::LiquidityManager;

pub struct RpcServer {
    port: u16,
    db: Database,
    shielded_pool: Arc<RwLock<ShieldedPoolManager>>,
    token_registry: Arc<TokenRegistry>,
    liquidity_manager: Arc<RwLock<LiquidityManager>>,
}

// ============ Request/Response Types ============

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositNotification {
    pub deposit_id: String,
    pub source_chain_id: u64,
    pub target_chain_id: u64,
    pub sender: String,
    pub token: String,
    pub amount: u64,
    pub recipient: Vec<u8>,
    pub zcash_address: Vec<u8>,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WithdrawalNotification {
    pub withdrawal_id: String,
    pub target_chain_id: u64,
    pub recipient: String,
    pub token: String,
    pub amount: u64,
    pub nullifier: Vec<u8>,
    pub zcash_proof: Vec<u8>,
    pub merkle_root: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthorizedWithdrawal {
    pub withdrawal_id: String,
    pub target_chain_id: u64,
    pub recipient: String,
    pub token: String,
    pub amount: u64,
    pub nullifier: Vec<u8>,
    pub authorization_signature: Vec<u8>,
}

#[derive(Serialize)]
struct StatusResponse {
    status: String,
}

#[derive(Serialize)]
struct DepositStatusResponse {
    deposit_id: String,
    processed: bool,
    zcash_txid: Option<String>,
    note_commitment: Option<String>,
}

#[derive(Serialize)]
struct LiquidityCheckResponse {
    available: bool,
    current_liquidity: u64,
}

#[derive(Deserialize)]
struct LiquidityCheckRequest {
    chain_id: u64,
    token: String,
    amount: u64,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    zcash_synced: bool,
}

#[derive(Serialize)]
struct StatsResponse {
    total_deposits: u64,
    total_withdrawals: u64,
    total_volume: u64,
    active_deposits: u64,
}

// ============ Server State ============

#[derive(Clone)]
struct AppState {
    db: Database,
    shielded_pool: Arc<RwLock<ShieldedPoolManager>>,
    token_registry: Arc<TokenRegistry>,
    liquidity_manager: Arc<RwLock<LiquidityManager>>,
}

impl RpcServer {
    pub fn new(
        port: u16,
        db: Database,
        shielded_pool: Arc<RwLock<ShieldedPoolManager>>,
        token_registry: Arc<TokenRegistry>,
        liquidity_manager: Arc<RwLock<LiquidityManager>>,
    ) -> Self {
        Self {
            port,
            db,
            shielded_pool,
            token_registry,
            liquidity_manager,
        }
    }
    
    pub async fn start(self) -> anyhow::Result<()> {
        let state = AppState {
            db: self.db,
            shielded_pool: self.shielded_pool,
            token_registry: self.token_registry,
            liquidity_manager: self.liquidity_manager,
        };
        
        let app = Router::new()
            // Health & status
            .route("/health", get(health_handler))
            .route("/stats", get(stats_handler))
            
            // Deposit endpoints (relayers notify us)
            .route("/deposits/notify", post(notify_deposit_handler))
            .route("/deposits/:id/status", get(deposit_status_handler))
            
            // Withdrawal endpoints
            .route("/withdrawals/notify", post(notify_withdrawal_handler))
            .route("/withdrawals/authorized", get(authorized_withdrawals_handler))
            
            // Liquidity endpoints
            .route("/liquidity/check", post(check_liquidity_handler))
            
            .with_state(state);
        
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        
        info!("RPC server listening on {}", addr);
        
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

// ============ Handlers ============

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        zcash_synced: true,
    })
}

async fn stats_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<StatsResponse>, StatusCode> {
    let stats = state.db.get_stats().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(StatsResponse {
        total_deposits: stats.total_deposits,
        total_withdrawals: stats.total_withdrawals,
        total_volume: stats.total_volume,
        active_deposits: stats.active_deposits,
    }))
}

/// Relayer notifies coordinator about a new deposit
/// Coordinator will create the Zcash shielded note
async fn notify_deposit_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(notification): Json<DepositNotification>,
) -> Result<Json<StatusResponse>, StatusCode> {
    info!("Received deposit notification from relayer: {}", notification.deposit_id);
    
    // Store in database for processing
    let deposit = Deposit {
        deposit_id: notification.deposit_id.clone(),
        source_chain_id: notification.source_chain_id,
        target_chain_id: notification.target_chain_id,
        sender: notification.sender,
        recipient: notification.recipient,
        token: notification.token,
        amount: notification.amount,
        zcash_address: notification.zcash_address,
        processed: false,
        zcash_txid: None,
        note_commitment: None,
        created_at: notification.timestamp as i64,
    };
    
    state.db.store_deposit(&deposit).await
        .map_err(|e| {
            warn!("Failed to store deposit: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    info!("Deposit queued for processing: {}", notification.deposit_id);
    
    Ok(Json(StatusResponse {
        status: "queued".to_string(),
    }))
}

async fn deposit_status_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(deposit_id): Path<String>,
) -> Result<Json<DepositStatusResponse>, StatusCode> {
    let deposits = state.db.get_pending_deposits().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let deposit = deposits.iter().find(|d| d.deposit_id == deposit_id);
    
    match deposit {
        Some(d) => Ok(Json(DepositStatusResponse {
            deposit_id: d.deposit_id.clone(),
            processed: d.processed,
            zcash_txid: d.zcash_txid.clone(),
            note_commitment: d.note_commitment.clone(),
        })),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Relayer notifies coordinator about a withdrawal request
/// Coordinator will verify the proof and authorize
async fn notify_withdrawal_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(notification): Json<WithdrawalNotification>,
) -> Result<Json<StatusResponse>, StatusCode> {
    info!("Received withdrawal notification from relayer: {}", notification.withdrawal_id);
    
    // Store in database for verification
    let withdrawal = Withdrawal {
        withdrawal_id: notification.withdrawal_id.clone(),
        target_chain_id: notification.target_chain_id,
        recipient: notification.recipient,
        token: notification.token,
        amount: notification.amount,
        nullifier: notification.nullifier,
        zcash_proof: notification.zcash_proof,
        merkle_root: notification.merkle_root,
        authorized: false,
        auth_signature: None,
        created_at: chrono::Utc::now().timestamp(),
    };
    
    state.db.store_withdrawal(&withdrawal).await
        .map_err(|e| {
            warn!("Failed to store withdrawal: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    info!("Withdrawal queued for verification: {}", notification.withdrawal_id);
    
    Ok(Json(StatusResponse {
        status: "queued".to_string(),
    }))
}

/// Relayer queries for authorized withdrawals ready to execute
/// Coordinator has already verified proofs and signed authorization
async fn authorized_withdrawals_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<AuthorizedWithdrawal>>, StatusCode> {
    let authorized = state.db.get_authorized_withdrawals().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let results: Vec<AuthorizedWithdrawal> = authorized
        .into_iter()
        .filter_map(|w| {
            w.auth_signature.map(|sig| AuthorizedWithdrawal {
                withdrawal_id: w.withdrawal_id,
                target_chain_id: w.target_chain_id,
                recipient: w.recipient,
                token: w.token,
                amount: w.amount,
                nullifier: w.nullifier.clone(),
                authorization_signature: sig,
            })
        })
        .collect();
    
    if !results.is_empty() {
        info!("Returning {} authorized withdrawals to relayer", results.len());
    }
    
    Ok(Json(results))
}

async fn check_liquidity_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(request): Json<LiquidityCheckRequest>,
) -> Result<Json<LiquidityCheckResponse>, StatusCode> {
    let liquidity_manager = state.liquidity_manager.read().await;
    
    match liquidity_manager.get_pool(request.chain_id, &request.token) {
        Some(pool) => {
            let available = pool.available >= request.amount;
            Ok(Json(LiquidityCheckResponse {
                available,
                current_liquidity: pool.available,
            }))
        }
        None => Ok(Json(LiquidityCheckResponse {
            available: false,
            current_liquidity: 0,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_endpoint() {
        let response = health_handler().await;
        assert_eq!(response.status, "ok");
    }
}