use starknet::ContractAddress;

#[derive(Drop, Serde, starknet::Store)]
pub struct DepositInfo {
    pub deposit_id: felt252,
    pub sender: ContractAddress,
    pub token: ContractAddress,
    pub amount: u256,
    pub target_chain_id: u64,
    pub recipient: felt252,
    pub zcash_address: felt252,
    pub timestamp: u64,
    pub processed: bool,
}

#[derive(Drop, Serde, starknet::Store)]
pub struct WithdrawalRequestInfo {
    pub withdrawal_id: felt252,
    pub recipient: ContractAddress,
    pub token: ContractAddress,
    pub amount: u256,
    pub nullifier: felt252,
    pub timestamp: u64,
    pub executed: bool,
}

#[derive(Drop, Serde)]
pub struct BridgeStats {
    pub total_deposits: u256,
    pub total_withdrawals: u256,
    pub total_volume: u256,
    pub active_deposits: u256,
}