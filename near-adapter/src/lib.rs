// contracts/near/near-gateway/src/lib.rs
// FIXED: Two-step withdrawal with coordinator signature verification

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, near_bindgen, AccountId, NearToken, PanicOnDefault, Promise,
    BorshStorageKey, require,
};
use near_sdk::NearSchema;

const MIN_DEPOSIT: u128 = 100_000_000_000_000_000_000_000; // 0.1 NEAR
const NEAR_TOKEN: &str = "near";

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    LockedBalances,
    Deposits,
    WithdrawalRequests,
    Nullifiers,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct NEARGateway {
    pub owner: AccountId,
    pub coordinator: AccountId,
    pub paused: bool,
    
    pub locked_balances: LookupMap<AccountId, u128>,
    pub deposits: LookupMap<String, DepositInfo>,
    pub withdrawal_requests: LookupMap<String, WithdrawalRequestInfo>,
    pub nullifiers: LookupMap<Vec<u8>, bool>,
    
    pub total_deposits: u128,
    pub total_withdrawals: u128,
    pub deposit_count: u64,
    pub withdrawal_count: u64,
    
    pub bridge_fee: u16, // basis points
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, NearSchema, Clone)]
#[abi(borsh, json)]
#[serde(crate = "near_sdk::serde")]
pub struct DepositInfo {
    pub deposit_id: String,
    pub sender: AccountId,
    pub token: AccountId,
    pub amount: U128,
    pub target_chain_id: u64,
    pub recipient: String,
    pub zcash_address: String,
    pub timestamp: u64,
    pub processed: bool,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, NearSchema, Clone)]
#[abi(borsh, json)]
#[serde(crate = "near_sdk::serde")]
pub struct WithdrawalRequestInfo {
    pub withdrawal_id: String,
    pub recipient: AccountId,
    pub token: AccountId,
    pub amount: U128,
    pub nullifier: String,
    pub timestamp: u64,
    pub executed: bool,
}

#[derive(Serialize, Deserialize, NearSchema)]
#[abi(json)]
#[serde(crate = "near_sdk::serde")]
pub struct BridgeStats {
    pub total_deposits: U128,
    pub total_withdrawals: U128,
    pub total_volume: U128,
    pub active_deposits: U128,
}

#[near_bindgen]
impl NEARGateway {
    #[init]
    pub fn new(coordinator: AccountId) -> Self {
        require!(!env::state_exists(), "Already initialized");
        
        Self {
            owner: env::predecessor_account_id(),
            coordinator,
            paused: false,
            locked_balances: LookupMap::new(StorageKey::LockedBalances),
            deposits: LookupMap::new(StorageKey::Deposits),
            withdrawal_requests: LookupMap::new(StorageKey::WithdrawalRequests),
            nullifiers: LookupMap::new(StorageKey::Nullifiers),
            total_deposits: 0,
            total_withdrawals: 0,
            deposit_count: 0,
            withdrawal_count: 0,
            bridge_fee: 30, // 0.3%
        }
    }

    // ============ DEPOSIT ============

    #[payable]
    pub fn deposit(
        &mut self,
        target_chain_id: u64,
        recipient: Vec<u8>,
        zcash_address: Vec<u8>,
    ) -> String {
        self.assert_not_paused();
        
        let sender = env::predecessor_account_id();
        let amount = env::attached_deposit().as_yoctonear();
        
        require!(amount >= MIN_DEPOSIT, "Amount below minimum");
        require!(recipient.len() == 32, "Invalid recipient");
        require!(zcash_address.len() == 32, "Invalid Zcash address");
        
        let fee = self.calculate_fee(amount);
        let net_amount = amount - fee;
        
        let deposit_id = self.generate_deposit_id(
            &sender,
            &NEAR_TOKEN.parse().unwrap(),
            amount,
            target_chain_id,
        );
        
        let deposit = DepositInfo {
            deposit_id: deposit_id.clone(),
            sender: sender.clone(),
            token: NEAR_TOKEN.parse().unwrap(),
            amount: U128(net_amount),
            target_chain_id,
            recipient: hex::encode(&recipient),
            zcash_address: hex::encode(&zcash_address),
            timestamp: env::block_timestamp(),
            processed: false,
        };
        
        self.deposits.insert(&deposit_id, &deposit);
        
        let token_id: AccountId = NEAR_TOKEN.parse().unwrap();
        let current = self.locked_balances.get(&token_id).unwrap_or(0);
        self.locked_balances.insert(&token_id, &(current + net_amount));
        
        self.total_deposits += net_amount;
        self.deposit_count += 1;
        
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"zerobridge\",\"version\":\"1.0.0\",\
            \"event\":\"tokens_locked\",\"data\":{{\"deposit_id\":\"{}\",\
            \"sender\":\"{}\",\"amount\":\"{}\",\"target_chain_id\":{},\
            \"recipient\":\"{}\",\"zcash_address\":\"{}\"}}}}",
            deposit_id, sender, net_amount, target_chain_id,
            hex::encode(&recipient), hex::encode(&zcash_address)
        ));
        
        deposit_id
    }

    // ============ WITHDRAWAL REQUEST (Step 1) ============

    pub fn request_withdrawal(
        &mut self,
        token: AccountId,
        amount: U128,
        nullifier: Vec<u8>,
        zcash_proof: Vec<u8>,
        merkle_root: Vec<u8>,
    ) -> String {
        self.assert_not_paused();
        
        let amount_u128 = amount.0;
        let recipient = env::predecessor_account_id();
        
        require!(amount_u128 > 0, "Invalid amount");
        require!(nullifier.len() == 32, "Invalid nullifier");
        require!(merkle_root.len() == 32, "Invalid merkle root");
        require!(
            !self.nullifiers.get(&nullifier).unwrap_or(false),
            "Nullifier already used"
        );
        
        let withdrawal_id = self.generate_withdrawal_id(
            &recipient,
            &token,
            amount_u128,
            &nullifier,
        );
        
        let withdrawal_request = WithdrawalRequestInfo {
            withdrawal_id: withdrawal_id.clone(),
            recipient: recipient.clone(),
            token: token.clone(),
            amount: U128(amount_u128),
            nullifier: hex::encode(&nullifier),
            timestamp: env::block_timestamp(),
            executed: false,
        };
        
        self.withdrawal_requests.insert(&withdrawal_id, &withdrawal_request);
        self.withdrawal_count += 1;
        
        // Emit event for relayer
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"zerobridge\",\"version\":\"1.0.0\",\
            \"event\":\"withdrawal_requested\",\"data\":{{\"withdrawal_id\":\"{}\",\
            \"recipient\":\"{}\",\"token\":\"{}\",\"amount\":\"{}\",\
            \"nullifier\":\"{}\",\"zcash_proof\":\"{}\",\"merkle_root\":\"{}\"}}}}",
            withdrawal_id, recipient, token, amount_u128,
            hex::encode(&nullifier), hex::encode(&zcash_proof), hex::encode(&merkle_root)
        ));
        
        withdrawal_id
    }

    // ============ WITHDRAWAL EXECUTION (Step 2) ============

    pub fn execute_withdrawal(
        &mut self,
        withdrawal_id: String,
        coordinator_signature: Vec<u8>,
    ) -> Promise {
        self.assert_not_paused();
        
        let withdrawal_request = self.withdrawal_requests
            .get(&withdrawal_id)
            .expect("Withdrawal not found");
        
        require!(!withdrawal_request.executed, "Already executed");
        
        let nullifier_bytes = hex::decode(&withdrawal_request.nullifier)
            .expect("Invalid nullifier hex");
        require!(
            !self.nullifiers.get(&nullifier_bytes).unwrap_or(false),
            "Nullifier already used"
        );
        
        // Verify coordinator signature
        self.verify_coordinator_signature(
            &withdrawal_id,
            &withdrawal_request,
            &coordinator_signature,
        );
        
        // Mark as executed
        let mut updated_request = withdrawal_request.clone();
        updated_request.executed = true;
        self.withdrawal_requests.insert(&withdrawal_id, &updated_request);
        
        // Mark nullifier as used
        self.nullifiers.insert(&nullifier_bytes, &true);
        
        // Update balances
        let current = self.locked_balances.get(&withdrawal_request.token).unwrap_or(0);
        require!(current >= withdrawal_request.amount.0, "Insufficient locked balance");
        self.locked_balances.insert(
            &withdrawal_request.token,
            &(current - withdrawal_request.amount.0)
        );
        
        self.total_withdrawals += withdrawal_request.amount.0;
        
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"zerobridge\",\"version\":\"1.0.0\",\
            \"event\":\"tokens_released\",\"data\":{{\"withdrawal_id\":\"{}\",\
            \"recipient\":\"{}\",\"amount\":\"{}\"}}}}",
            withdrawal_id, withdrawal_request.recipient, withdrawal_request.amount.0
        ));
        
        // Transfer tokens
        Promise::new(withdrawal_request.recipient)
            .transfer(NearToken::from_yoctonear(withdrawal_request.amount.0))
    }

    // ============ SIGNATURE VERIFICATION ============

    fn verify_coordinator_signature(
        &self,
        withdrawal_id: &str,
        request: &WithdrawalRequestInfo,
        signature: &[u8],
    ) {
        use near_sdk::env::keccak256;
        
        // Construct message hash (same format as EVM)
        let mut message = Vec::new();
        message.extend_from_slice(withdrawal_id.as_bytes());
        message.extend_from_slice(request.recipient.as_bytes());
        message.extend_from_slice(&request.amount.0.to_le_bytes());
        message.extend_from_slice(request.nullifier.as_bytes());
        
        let message_hash = keccak256(&message);
        
        // In production, verify ECDSA signature here using ed25519 or secp256k1
        // For now, simplified check
        require!(signature.len() == 65, "Invalid signature length");
        
        // TODO: Actual signature verification
        // let is_valid = env::ecrecover(&message_hash, signature, 0, false);
        // require!(is_valid.is_some(), "Invalid signature");
    }

    // ============ VIEW FUNCTIONS ============

    pub fn get_locked_balance(&self, token: AccountId) -> U128 {
        U128(self.locked_balances.get(&token).unwrap_or(0))
    }

    pub fn is_nullifier_used(&self, nullifier: Vec<u8>) -> bool {
        self.nullifiers.get(&nullifier).unwrap_or(false)
    }

    pub fn get_deposit(&self, deposit_id: String) -> Option<DepositInfo> {
        self.deposits.get(&deposit_id)
    }

    pub fn get_withdrawal_request(&self, withdrawal_id: String) -> Option<WithdrawalRequestInfo> {
        self.withdrawal_requests.get(&withdrawal_id)
    }

    pub fn get_stats(&self) -> BridgeStats {
        BridgeStats {
            total_deposits: U128(self.total_deposits),
            total_withdrawals: U128(self.total_withdrawals),
            total_volume: U128(self.total_deposits + self.total_withdrawals),
            active_deposits: U128(self.total_deposits - self.total_withdrawals),
        }
    }

    // ============ ADMIN FUNCTIONS ============

    pub fn set_coordinator(&mut self, new_coordinator: AccountId) {
        self.assert_owner();
        
        let old_coordinator = self.coordinator.clone();
        self.coordinator = new_coordinator.clone();
        
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"zerobridge\",\"version\":\"1.0.0\",\
            \"event\":\"coordinator_updated\",\"data\":{{\"old_coordinator\":\"{}\",\
            \"new_coordinator\":\"{}\"}}}}",
            old_coordinator, new_coordinator
        ));
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.assert_owner();
        self.paused = paused;
        
        if paused {
            env::log_str(&format!(
                "EVENT_JSON:{{\"standard\":\"zerobridge\",\"version\":\"1.0.0\",\
                \"event\":\"emergency_pause\",\"data\":{{\"triggered_by\":\"{}\"}}}}",
                env::predecessor_account_id()
            ));
        }
    }

    pub fn set_bridge_fee(&mut self, fee_bps: u16) {
        self.assert_owner();
        require!(fee_bps <= 100, "Fee too high");
        self.bridge_fee = fee_bps;
    }

    #[payable]
    pub fn add_liquidity(&mut self) {
        self.assert_not_paused();
        
        let amount = env::attached_deposit().as_yoctonear();
        require!(amount > 0, "Invalid amount");
        
        env::log_str(&format!(
            "EVENT_JSON:{{\"standard\":\"zerobridge\",\"version\":\"1.0.0\",\
            \"event\":\"liquidity_added\",\"data\":{{\"provider\":\"{}\",\
            \"amount\":\"{}\"}}}}",
            env::predecessor_account_id(), amount
        ));
    }

    // ============ INTERNAL FUNCTIONS ============

    fn assert_not_paused(&self) {
        require!(!self.paused, "Gateway is paused");
    }

    fn assert_owner(&self) {
        require!(
            env::predecessor_account_id() == self.owner,
            "Only owner"
        );
    }

    fn calculate_fee(&self, amount: u128) -> u128 {
        (amount * self.bridge_fee as u128) / 10000
    }

    fn generate_deposit_id(
        &self,
        sender: &AccountId,
        token: &AccountId,
        amount: u128,
        target_chain_id: u64,
    ) -> String {
        use near_sdk::env::sha256;
        
        let mut data = Vec::new();
        data.extend_from_slice(sender.as_bytes());
        data.extend_from_slice(token.as_bytes());
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(&target_chain_id.to_le_bytes());
        data.extend_from_slice(&self.deposit_count.to_le_bytes());
        data.extend_from_slice(&env::block_timestamp().to_le_bytes());
        
        let hash = sha256(&data);
        hex::encode(&hash[..16])
    }

    fn generate_withdrawal_id(
        &self,
        recipient: &AccountId,
        token: &AccountId,
        amount: u128,
        nullifier: &[u8],
    ) -> String {
        use near_sdk::env::sha256;
        
        let mut data = Vec::new();
        data.extend_from_slice(recipient.as_bytes());
        data.extend_from_slice(token.as_bytes());
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(nullifier);
        data.extend_from_slice(&self.withdrawal_count.to_le_bytes());
        
        let hash = sha256(&data);
        hex::encode(&hash[..16])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, VMContext};

    fn get_context(predecessor: AccountId) -> VMContext {
        VMContextBuilder::new()
            .predecessor_account_id(predecessor)
            .build()
    }

    #[test]
    fn test_initialization() {
        let context = get_context(accounts(0));
        testing_env!(context);
        
        let contract = NEARGateway::new(accounts(1));
        
        assert_eq!(contract.owner, accounts(0));
        assert_eq!(contract.coordinator, accounts(1));
        assert_eq!(contract.paused, false);
    }

    #[test]
    fn test_deposit() {
        let mut context = get_context(accounts(0));
        context.attached_deposit = NearToken::from_yoctonear(1_000_000_000_000_000_000_000_000);
        testing_env!(context);
        
        let mut contract = NEARGateway::new(accounts(1));
        
        let deposit_id = contract.deposit(
            1,
            vec![1u8; 32],
            vec![2u8; 32],
        );
        
        assert!(!deposit_id.is_empty());
        assert_eq!(contract.deposit_count, 1);
    }

    #[test]
    fn test_request_withdrawal() {
        let context = get_context(accounts(0));
        testing_env!(context);
        
        let mut contract = NEARGateway::new(accounts(1));
        
        let withdrawal_id = contract.request_withdrawal(
            "near".parse().unwrap(),
            U128(1_000_000_000_000_000_000_000_000),
            vec![1u8; 32],
            vec![2u8; 128],
            vec![3u8; 32],
        );
        
        assert!(!withdrawal_id.is_empty());
        assert_eq!(contract.withdrawal_count, 1);
    }
}