// relayer/src/p2p_network.rs
//! P2P gossip network for relayer coordination
//! FOCUSED: Prevent duplicate work, coordinate task claiming

use anyhow::Result;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, debug};

use crate::config::RelayerConfig;
use crate::stake_manager::StakeManager;

pub struct P2PNetwork {
    config: RelayerConfig,
    _stake_manager: Arc<StakeManager>,
    task_claims: Arc<RwLock<HashMap<String, TaskClaim>>>,
}

#[derive(Debug, Clone)]
struct TaskClaim {
    task_id: String,
    claimed_by: String,
    claimed_at: i64,
    expires_at: i64,
}

impl P2PNetwork {
    pub async fn new(
        config: RelayerConfig,
        stake_manager: Arc<StakeManager>,
    ) -> Result<Self> {
        Ok(Self {
            config,
            _stake_manager: stake_manager,
            task_claims: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting P2P network on {}:{}", 
            self.config.p2p.listen_addr, 
            self.config.p2p.port
        );
        
        // In production, initialize libp2p here with:
        // - GossipSub for message broadcasting
        // - Kademlia for peer discovery
        // - QUIC transport
        // - Noise encryption
        
        info!("P2P network initialized with {} bootstrap peers", 
            self.config.p2p.bootstrap_peers.len()
        );
        
        Ok(())
    }

    /// Send heartbeat to peers
    pub async fn send_heartbeat(&self) -> Result<()> {
        debug!("Sending P2P heartbeat");
        
        // Broadcast heartbeat message to peers
        // Contains: relayer ID, stake amount, reputation
        
        Ok(())
    }

    /// Check if a task is already claimed by another relayer
    pub async fn is_task_claimed(&self, task_id: &str) -> Result<bool> {
        let claims = self.task_claims.read().await;
        
        if let Some(claim) = claims.get(task_id) {
            let now = chrono::Utc::now().timestamp();
            
            // Check if claim is still valid
            if claim.expires_at > now {
                debug!("Task {} already claimed by {}", task_id, claim.claimed_by);
                return Ok(true);
            }
        }
        
        Ok(false)
    }

    /// Broadcast task claim to network
    /// This prevents other relayers from claiming the same task
    pub async fn broadcast_task_claim(&self, task_id: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + 300; // 5 minute claim
        
        let claim = TaskClaim {
            task_id: task_id.to_string(),
            claimed_by: self.config.relayer_identity.address.clone(),
            claimed_at: now,
            expires_at,
        };
        
        // Store locally
        {
            let mut claims = self.task_claims.write().await;
            claims.insert(task_id.to_string(), claim.clone());
        }
        
        // Broadcast to P2P network
        info!("Broadcasting task claim: {}", task_id);
        self.gossip_message(&format!("CLAIM:{}", task_id)).await?;
        
        Ok(())
    }

    /// Broadcast withdrawal execution completion
    /// This notifies other relayers that the task is done
    pub async fn broadcast_withdrawal_execution(
        &self,
        withdrawal_id: &str,
        tx_hash: &str,
    ) -> Result<()> {
        info!(
            "Broadcasting withdrawal execution: {} -> {}",
            withdrawal_id, tx_hash
        );
        
        // Remove from claims
        {
            let mut claims = self.task_claims.write().await;
            claims.remove(withdrawal_id);
        }
        
        // Broadcast to network
        self.gossip_message(&format!("EXECUTED:{}:{}", withdrawal_id, tx_hash))
            .await?;
        
        Ok(())
    }

    /// Broadcast deposit notification
    /// This tells other relayers we've notified the coordinator
    pub async fn broadcast_deposit_notification(&self, deposit_id: &str) -> Result<()> {
        debug!("Broadcasting deposit notification: {}", deposit_id);
        
        self.gossip_message(&format!("DEPOSIT_NOTIFIED:{}", deposit_id))
            .await?;
        
        Ok(())
    }

    /// Handle incoming P2P message from another relayer
    pub async fn handle_incoming_message(&self, message: &str) -> Result<()> {
        debug!("Received P2P message: {}", message);
        
        if message.starts_with("CLAIM:") {
            // Another relayer claimed a task
            let task_id = &message[6..];
            self.handle_claim_message(task_id).await?;
        } else if message.starts_with("EXECUTED:") {
            // Another relayer executed a withdrawal
            let parts: Vec<&str> = message[9..].split(':').collect();
            if parts.len() == 2 {
                self.handle_execution_message(parts[0], parts[1]).await?;
            }
        } else if message.starts_with("DEPOSIT_NOTIFIED:") {
            // Another relayer notified coordinator about deposit
            let deposit_id = &message[17..];
            debug!("Deposit {} already notified by peer", deposit_id);
        }
        
        Ok(())
    }

    /// Handle claim message from another relayer
    async fn handle_claim_message(&self, task_id: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + 300;
        
        let claim = TaskClaim {
            task_id: task_id.to_string(),
            claimed_by: "peer".to_string(), // Would be actual peer ID
            claimed_at: now,
            expires_at,
        };
        
        let mut claims = self.task_claims.write().await;
        claims.insert(task_id.to_string(), claim);
        
        Ok(())
    }

    /// Handle execution message from another relayer
    async fn handle_execution_message(
        &self,
        withdrawal_id: &str,
        _tx_hash: &str,
    ) -> Result<()> {
        // Remove from our claims
        let mut claims = self.task_claims.write().await;
        claims.remove(withdrawal_id);
        
        Ok(())
    }

    /// Cleanup expired claims
    pub async fn cleanup_expired_claims(&self) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        
        let mut claims = self.task_claims.write().await;
        claims.retain(|_, claim| claim.expires_at > now);
        
        Ok(())
    }

    /// Gossip message to all peers
    async fn gossip_message(&self, message: &str) -> Result<()> {
        // In production, use libp2p GossipSub to broadcast
        debug!("Gossiping message: {}", message);
        
        // This would publish to a topic like:
        // gossipsub.publish("zerobridge-relayers", message.as_bytes())
        
        Ok(())
    }

    /// Get current number of connected peers
    pub async fn peer_count(&self) -> usize {
        // In production, query libp2p peer store
        self.config.p2p.bootstrap_peers.len()
    }

    /// Get network statistics
    pub async fn network_stats(&self) -> NetworkStats {
        let claims = self.task_claims.read().await;
        
        NetworkStats {
            connected_peers: self.peer_count().await,
            active_claims: claims.len(),
            bootstrap_peers: self.config.p2p.bootstrap_peers.len(),
        }
    }
}

#[derive(Debug)]
pub struct NetworkStats {
    pub connected_peers: usize,
    pub active_claims: usize,
    pub bootstrap_peers: usize,
}