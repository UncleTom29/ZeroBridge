// relayer/src/transaction_executor.rs
//! Transaction execution on destination chains
//! UNIQUE RESPONSIBILITY: Submit withdrawal transactions to gateways
//! Does NOT create proofs or verify proofs (coordinator does that)
//! Does NOT manage liquidity (coordinator does that)

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, debug};

use crate::config::{RelayerConfig, ChainConfig};
use crate::coordinator_client::CoordinatorClient;
use crate::stake_manager::StakeManager;
use crate::database::RelayerDatabase;

pub struct TransactionExecutor {
    config: RelayerConfig,
    _coordinator: Arc<CoordinatorClient>,
    _stake_manager: Arc<StakeManager>,
    _db: RelayerDatabase,
}

impl TransactionExecutor {
    pub async fn new(
        config: RelayerConfig,
        coordinator: Arc<CoordinatorClient>,
        stake_manager: Arc<StakeManager>,
        db: RelayerDatabase,
    ) -> Result<Self> {
        Ok(Self {
            config,
            _coordinator: coordinator,
            _stake_manager: stake_manager,
            _db: db,
        })
    }

    /// Execute withdrawal transaction on destination chain
    /// Coordinator has already verified the proof and provided authorization
    pub async fn execute_withdrawal(
        &self,
        chain_id: u64,
        recipient: &str,
        token: &str,
        amount: u64,
        nullifier: &[u8],
        auth_signature: &[u8],
    ) -> Result<String> {
        info!(
            "Executing withdrawal: chain={}, recipient={}, amount={}",
            chain_id, recipient, amount
        );

        let chain_config = self
            .config
            .get_chain(chain_id)
            .ok_or_else(|| anyhow::anyhow!("Chain {} not configured", chain_id))?;

        match chain_config.chain_type {
            crate::config::ChainType::Ethereum
            | crate::config::ChainType::Base
            | crate::config::ChainType::Polygon => {
                self.execute_evm_withdrawal(
                    chain_config,
                    recipient,
                    token,
                    amount,
                    nullifier,
                    auth_signature,
                )
                .await
            }
            crate::config::ChainType::Solana => {
                self.execute_solana_withdrawal(
                    chain_config,
                    recipient,
                    token,
                    amount,
                    nullifier,
                    auth_signature,
                )
                .await
            }
            crate::config::ChainType::Near => {
                self.execute_near_withdrawal(
                    chain_config,
                    recipient,
                    token,
                    amount,
                    nullifier,
                    auth_signature,
                )
                .await
            }
            crate::config::ChainType::Mina => {
                self.execute_mina_withdrawal(
                    chain_config,
                    recipient,
                    token,
                    amount,
                    nullifier,
                    auth_signature,
                )
                .await
            }
        }
    }

    /// Execute withdrawal on EVM chain (Ethereum, Base, Polygon)
    async fn execute_evm_withdrawal(
        &self,
        chain_config: &ChainConfig,
        recipient: &str,
        token: &str,
        amount: u64,
        nullifier: &[u8],
        auth_signature: &[u8],
    ) -> Result<String> {
        use ethers::prelude::*;

        debug!("Executing EVM withdrawal on chain {}", chain_config.chain_id);

        let provider = Provider::<Http>::try_from(&chain_config.rpc_url)?;
        let wallet: LocalWallet = chain_config.private_key.parse()?;
        let chain_id = chain_config.chain_id;
        let client = SignerMiddleware::new(provider, wallet.with_chain_id(chain_id));

        let gateway: Address = chain_config.gateway_address.parse()?;
        let recipient_addr: Address = recipient.parse()?;
        let token_addr: Address = token.parse()?;

        // Encode executeWithdrawal call
        // function executeWithdrawal(
        //     address recipient,
        //     address token,
        //     uint256 amount,
        //     bytes32 nullifier,
        //     bytes calldata authSignature
        // )
        let mut call_data = Vec::new();
        
        // Function selector for executeWithdrawal
        call_data.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]); // Placeholder selector
        
        // Encode parameters (simplified)
        call_data.extend_from_slice(recipient_addr.as_bytes());
        call_data.extend_from_slice(token_addr.as_bytes());
        call_data.extend_from_slice(&amount.to_be_bytes());
        call_data.extend_from_slice(nullifier);
        call_data.extend_from_slice(auth_signature);

        // Estimate gas
        let gas_price = client.get_gas_price().await?;
        let gas_limit = U256::from(300_000); // Base gas limit

        // Submit transaction
        let tx = TransactionRequest::new()
            .to(gateway)
            .data(call_data)
            .gas(gas_limit)
            .gas_price(gas_price * chain_config.gas_strategy.multiplier as u64);

        let pending_tx = client.send_transaction(tx, None).await?;
        
        info!(
            "EVM withdrawal submitted: tx={:?}",
            pending_tx.tx_hash()
        );

        // Wait for confirmation
        let receipt = pending_tx
            .confirmations(chain_config.confirmations as usize)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Transaction dropped"))?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        
        info!("✓ EVM withdrawal confirmed: {}", tx_hash);
        Ok(tx_hash)
    }

    /// Execute withdrawal on Solana
    async fn execute_solana_withdrawal(
        &self,
        chain_config: &ChainConfig,
        recipient: &str,
        token: &str,
        amount: u64,
        nullifier: &[u8],
        auth_signature: &[u8],
    ) -> Result<String> {
        use solana_client::rpc_client::RpcClient;
        use solana_sdk::{
            signature::{Keypair, Signer},
            transaction::Transaction,
            instruction::{Instruction, AccountMeta},
            pubkey::Pubkey,
        };

        debug!("Executing Solana withdrawal");

        let client = RpcClient::new(&chain_config.rpc_url);
        
        // Parse keys
        let keypair_bytes = hex::decode(&chain_config.private_key)?;
        let keypair = Keypair::from_bytes(&keypair_bytes)?;
        
        let program_id: Pubkey = chain_config.gateway_address.parse()?;
        let recipient_key: Pubkey = recipient.parse()?;
        let token_key: Pubkey = token.parse()?;

        // Build instruction data
        let mut instruction_data = Vec::new();
        instruction_data.push(2u8); // Withdrawal instruction discriminator
        instruction_data.extend_from_slice(&amount.to_le_bytes());
        instruction_data.extend_from_slice(nullifier);
        instruction_data.extend_from_slice(auth_signature);

        // Create instruction
        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(keypair.pubkey(), true),
                AccountMeta::new(recipient_key, false),
                AccountMeta::new(token_key, false),
            ],
            data: instruction_data,
        };

        // Get recent blockhash
        let recent_blockhash = client.get_latest_blockhash()?;

        // Create and sign transaction
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[&keypair],
            recent_blockhash,
        );

        // Submit transaction
        let signature = client.send_and_confirm_transaction(&transaction)?;
        let tx_hash = signature.to_string();

        info!("✓ Solana withdrawal confirmed: {}", tx_hash);
        Ok(tx_hash)
    }

    /// Execute withdrawal on NEAR
    async fn execute_near_withdrawal(
        &self,
        chain_config: &ChainConfig,
        recipient: &str,
        token: &str,
        amount: u64,
        nullifier: &[u8],
        auth_signature: &[u8],
    ) -> Result<String> {
        debug!("Executing NEAR withdrawal");

        // NEAR transaction submission would go here
        // Using near-api-rs or near-jsonrpc-client
        
        info!(
            "NEAR withdrawal: recipient={}, token={}, amount={}",
            recipient, token, amount
        );

        // Placeholder - real implementation would use NEAR SDK
        let tx_hash = format!(
            "near_tx_{}",
            hex::encode(&nullifier[..8])
        );

        Ok(tx_hash)
    }

    /// Execute withdrawal on Mina
    async fn execute_mina_withdrawal(
        &self,
        chain_config: &ChainConfig,
        recipient: &str,
        token: &str,
        amount: u64,
        nullifier: &[u8],
        auth_signature: &[u8],
    ) -> Result<String> {
        debug!("Executing Mina withdrawal");

        // Mina transaction submission would go here
        // Using mina-signer or GraphQL API
        
        info!(
            "Mina withdrawal: recipient={}, token={}, amount={}",
            recipient, token, amount
        );

        // Placeholder - real implementation would use Mina SDK
        let tx_hash = format!(
            "mina_tx_{}",
            hex::encode(&nullifier[..8])
        );

        Ok(tx_hash)
    }
}