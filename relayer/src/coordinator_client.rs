// relayer/src/coordinator_client.rs
//! Client to query coordinator (read-only)
//! Relayer queries coordinator for authorization, doesn't duplicate coordinator logic

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub struct CoordinatorClient {
    base_url: String,
    client: reqwest::Client,
}

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
pub struct AuthorizedWithdrawal {
    pub withdrawal_id: String,
    pub target_chain_id: u64,
    pub recipient: String,
    pub token: String,
    pub amount: u64,
    pub nullifier: Vec<u8>,
    pub authorization_signature: Vec<u8>,
    pub timestamp: u64,
}

impl CoordinatorClient {
    pub fn new(base_url: &str) -> Result<Self> {
        Ok(Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        })
    }

    /// Notify coordinator about a deposit event
    /// Coordinator will create the Zcash note
    pub async fn notify_deposit(&self, deposit: DepositNotification) -> Result<()> {
        let url = format!("{}/deposits/notify", self.base_url);
        let response = self.client
            .post(&url)
            .json(&deposit)
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to notify deposit: {}", response.status());
        }
        
        Ok(())
    }

    /// Notify coordinator about a withdrawal request
    /// Coordinator will verify the proof and authorize if valid
    pub async fn notify_withdrawal(
        &self,
        withdrawal_id: &str,
        target_chain_id: u64,
        recipient: &str,
        token: &str,
        amount: u64,
        nullifier: Vec<u8>,
        zcash_proof: Vec<u8>,
        merkle_root: Vec<u8>,
    ) -> Result<()> {
        let url = format!("{}/withdrawals/notify", self.base_url);
        let response = self.client
            .post(&url)
            .json(&serde_json::json!({
                "withdrawal_id": withdrawal_id,
                "target_chain_id": target_chain_id,
                "recipient": recipient,
                "token": token,
                "amount": amount,
                "nullifier": nullifier,
                "zcash_proof": zcash_proof,
                "merkle_root": merkle_root,
            }))
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to notify withdrawal: {}", response.status());
        }
        
        Ok(())
    }

    /// Query for authorized withdrawals ready to be executed
    /// Coordinator has already verified proofs and authorized these
    pub async fn query_authorized_withdrawals(&self) -> Result<Vec<AuthorizedWithdrawal>> {
        let url = format!("{}/withdrawals/authorized", self.base_url);
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to query withdrawals: {}", response.status());
        }
        
        let withdrawals: Vec<AuthorizedWithdrawal> = response.json().await?;
        Ok(withdrawals)
    }

    /// Check if a specific deposit has been processed by coordinator
    pub async fn check_deposit_status(&self, deposit_id: &str) -> Result<bool> {
        let url = format!("{}/deposits/{}/status", self.base_url, deposit_id);
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Ok(false);
        }
        
        let status: serde_json::Value = response.json().await?;
        Ok(status["processed"].as_bool().unwrap_or(false))
    }

    /// Get liquidity status for a chain/token
    /// Coordinator manages liquidity, relayer just queries
    pub async fn check_liquidity(
        &self,
        chain_id: u64,
        token: &str,
        amount: u64,
    ) -> Result<bool> {
        let url = format!("{}/liquidity/check", self.base_url);
        let response = self.client
            .post(&url)
            .json(&serde_json::json!({
                "chain_id": chain_id,
                "token": token,
                "amount": amount,
            }))
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Ok(false);
        }
        
        let result: serde_json::Value = response.json().await?;
        Ok(result["available"].as_bool().unwrap_or(false))
    }
}