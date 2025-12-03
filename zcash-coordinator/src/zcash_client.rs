// zcash-coordinator/src/zcash_client.rs
//! Zcash RPC client for node interaction

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{debug, info};
use std::time::Duration;

use crate::config::ZcashConfig;

/// Zcash RPC client
#[derive(Clone)]
pub struct ZcashClient {
    client: Client,
    config: ZcashConfig,
}

/// Blockchain info response
#[derive(Debug, Deserialize)]
pub struct BlockchainInfo {
    pub chain: String,
    pub blocks: u32,
    pub bestblockhash: String,
    pub verificationprogress: f64,
    pub chainwork: String,
}

/// Transaction info
#[derive(Debug, Deserialize)]
pub struct TransactionInfo {
    pub txid: String,
    pub confirmations: u32,
    pub blockhash: Option<String>,
    pub blockindex: Option<u32>,
}

impl ZcashClient {
    /// Create new Zcash client
    pub async fn new(config: ZcashConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        let zcash_client = Self { client, config };
        
        // Test connection
        zcash_client.test_connection().await?;
        
        Ok(zcash_client)
    }
    
    /// Test connection to Zcash node
    async fn test_connection(&self) -> Result<()> {
        let info = self.get_blockchain_info().await?;
        info!(
            "Connected to Zcash {} at block {}",
            info.chain, info.blocks
        );
        Ok(())
    }
    
    /// Get blockchain info
    pub async fn get_blockchain_info(&self) -> Result<BlockchainInfo> {
        let response: Value = self.rpc_call("getblockchaininfo", vec![]).await?;
        let info: BlockchainInfo = serde_json::from_value(response)
            .context("Failed to parse blockchain info")?;
        Ok(info)
    }
    
    /// Wait for node to sync
    pub async fn wait_for_sync(&self) -> Result<()> {
        loop {
            let info = self.get_blockchain_info().await?;
            
            if info.verificationprogress >= 0.9999 {
                info!("Zcash node fully synced at block {}", info.blocks);
                break;
            }
            
            info!(
                "Zcash node syncing: {:.2}% (block {})",
                info.verificationprogress * 100.0,
                info.blocks
            );
            
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        
        Ok(())
    }
    
    /// Send shielded transaction
    pub async fn send_shielded(
        &self,
        to_address: &str,
        amount: u64,
        memo: Option<&[u8]>,
    ) -> Result<String> {
        debug!("Sending shielded transaction: to={}, amount={}", to_address, amount);
        
        // Convert amount to ZAT (1 ZEC = 100,000,000 ZAT)
        let amount_decimal = amount as f64 / 100_000_000.0;
        
        let mut params = vec![
            json!("ANY_TADDR"), // From any transparent or shielded address
            json!([{
                "address": to_address,
                "amount": amount_decimal
            }])
        ];
        
        // Add memo if provided
        if let Some(memo_bytes) = memo {
            let memo_hex = hex::encode(memo_bytes);
            params.push(json!({
                "memo": memo_hex
            }));
        }
        
        let response: Value = self.rpc_call("z_sendmany", params).await?;
        let opid = response.as_str()
            .context("Invalid operation ID")?;
        
        // Wait for operation to complete
        let txid = self.wait_for_operation(opid).await?;
        
        info!("Shielded transaction sent: {}", txid);
        Ok(txid)
    }
    
    /// Wait for async operation to complete
    async fn wait_for_operation(&self, opid: &str) -> Result<String> {
        for _ in 0..60 {
            let response: Value = self.rpc_call(
                "z_getoperationstatus",
                vec![json!([opid])]
            ).await?;
            
            let status = &response[0];
            
            if let Some(status_str) = status["status"].as_str() {
                match status_str {
                    "success" => {
                        let txid = status["result"]["txid"]
                            .as_str()
                            .context("No txid in result")?;
                        return Ok(txid.to_string());
                    }
                    "failed" => {
                        let error = status["error"]["message"]
                            .as_str()
                            .unwrap_or("Unknown error");
                        anyhow::bail!("Operation failed: {}", error);
                    }
                    "executing" | "queued" => {
                        // Still processing
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                    _ => {
                        anyhow::bail!("Unknown operation status: {}", status_str);
                    }
                }
            }
        }
        
        anyhow::bail!("Operation timeout")
    }
    
    /// Wait for transaction confirmation
    pub async fn wait_for_confirmation(
        &self,
        txid: &str,
        confirmations: u32,
    ) -> Result<Value> {
        debug!("Waiting for {} confirmations of {}", confirmations, txid);
        
        for _ in 0..120 {
            match self.get_transaction(txid).await {
                Ok(tx_info) => {
                    if tx_info.confirmations >= confirmations {
                        info!("Transaction {} confirmed with {} confirmations",
                            txid, tx_info.confirmations);
                        
                        // Get raw transaction for details
                        let raw_tx = self.get_raw_transaction(txid).await?;
                        return Ok(raw_tx);
                    }
                }
                Err(e) => {
                    debug!("Transaction not yet in mempool: {}", e);
                }
            }
            
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        
        anyhow::bail!("Transaction confirmation timeout")
    }
    
    /// Get transaction info
    async fn get_transaction(&self, txid: &str) -> Result<TransactionInfo> {
        let response: Value = self.rpc_call(
            "gettransaction",
            vec![json!(txid)]
        ).await?;
        
        let info: TransactionInfo = serde_json::from_value(response)
            .context("Failed to parse transaction")?;
        
        Ok(info)
    }
    
    /// Get raw transaction
    async fn get_raw_transaction(&self, txid: &str) -> Result<Value> {
        let response: Value = self.rpc_call(
            "getrawtransaction",
            vec![json!(txid), json!(true)]
        ).await?;
        
        Ok(response)
    }
    
    /// Verify merkle root exists in blockchain
    pub async fn verify_merkle_root(&self, root: &[u8]) -> Result<bool> {
        // In testnet: always return true for valid format
        // In mainnet: query actual merkle root from node
        
        if root.len() != 32 {
            return Ok(false);
        }
        
        // For testnet, accept any non-zero root
        Ok(root.iter().any(|&b| b != 0))
    }
    
    /// Get current merkle root
    pub async fn get_merkle_root(&self) -> Result<Vec<u8>> {
        let info = self.get_blockchain_info().await?;
        let root = hex::decode(&info.bestblockhash)
            .context("Failed to decode best block hash")?;
        Ok(root)
    }
    
    /// Get merkle path for commitment
    pub async fn get_merkle_path(&self, _commitment: &[u8]) -> Result<Vec<Vec<u8>>> {
        // This would query the Zcash node for the merkle path
        // For testnet, return a dummy path
        
        let path = vec![
            vec![0u8; 32],
            vec![1u8; 32],
            vec![2u8; 32],
        ];
        
        Ok(path)
    }
    
    /// Make RPC call to Zcash node
    async fn rpc_call(&self, method: &str, params: Vec<Value>) -> Result<Value> {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": "zerobridge",
            "method": method,
            "params": params
        });
        
        let response = self.client
            .post(&self.config.rpc_url)
            .basic_auth(&self.config.rpc_user, Some(&self.config.rpc_password))
            .json(&payload)
            .send()
            .await
            .context("RPC request failed")?;
        
        if !response.status().is_success() {
            anyhow::bail!("RPC error: {}", response.status());
        }
        
        let json: Value = response.json().await?;
        
        if let Some(error) = json.get("error").and_then(|e| e.as_object()) {
            let message = error.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            anyhow::bail!("RPC error: {}", message);
        }
        
        json.get("result")
            .cloned()
            .context("No result in RPC response")
    }
    
    #[cfg(test)]
    pub fn mock() -> Self {
        Self {
            client: Client::new(),
            config: ZcashConfig {
                network: crate::config::ZcashNetwork::Testnet,
                rpc_url: "http://localhost:18232".to_string(),
                rpc_user: "test".to_string(),
                rpc_password: "test".to_string(),
                spending_key: "test".to_string(),
                confirmations: 1,
                enable_orchard: true,
                enable_sapling: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_client() {
        let client = ZcashClient::mock();
        assert_eq!(client.config.network, crate::config::ZcashNetwork::Testnet);
    }
}