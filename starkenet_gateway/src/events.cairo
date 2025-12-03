use starknet::ContractAddress;
#[event]
#[derive(Drop, starknet::Event)]
pub enum Event {
    TokensLocked: TokensLocked,
    WithdrawalRequested: WithdrawalRequested,
    TokensReleased: TokensReleased,
    CoordinatorUpdated: CoordinatorUpdated,
    LiquidityAdded: LiquidityAdded,
    LiquidityRemoved: LiquidityRemoved,
    EmergencyPause: EmergencyPause,
    BridgeFeeUpdated: BridgeFeeUpdated,
}

#[derive(Drop, starknet::Event)]
pub struct TokensLocked {
    #[key] pub deposit_id: felt252,
    #[key] pub sender: ContractAddress,
    #[key] pub token: ContractAddress,
    pub amount: u256,
    pub target_chain_id: u64,
    pub recipient: felt252,
    pub zcash_address: felt252,
    pub timestamp: u64,
}

#[derive(Drop, starknet::Event)]
pub struct WithdrawalRequested {
    #[key] pub withdrawal_id: felt252,
    #[key] pub recipient: ContractAddress,
    #[key] pub token: ContractAddress,
    pub amount: u256,
    pub nullifier: felt252,
    pub timestamp: u64,
}

#[derive(Drop, starknet::Event)]
pub struct TokensReleased {
    #[key] pub withdrawal_id: felt252,
    #[key] pub recipient: ContractAddress,
    #[key] pub token: ContractAddress,
    pub amount: u256,
    pub nullifier: felt252,
    pub timestamp: u64,
}

#[derive(Drop, starknet::Event)]
pub struct CoordinatorUpdated {
    pub old_coordinator: ContractAddress,
    pub new_coordinator: ContractAddress,
    pub timestamp: u64,
}

#[derive(Drop, starknet::Event)]
pub struct LiquidityAdded {
    pub provider: ContractAddress,
    pub token: ContractAddress,
    pub amount: u256,
    pub timestamp: u64,
}

#[derive(Drop, starknet::Event)]
pub struct LiquidityRemoved {
    pub provider: ContractAddress,
    pub token: ContractAddress,
    pub amount: u256,
    pub timestamp: u64,
}

#[derive(Drop, starknet::Event)]
pub struct EmergencyPause {
    pub triggered_by: ContractAddress,
    pub reason: felt252,
    pub timestamp: u64,
}

#[derive(Drop, starknet::Event)]
pub struct BridgeFeeUpdated {
    pub old_fee: u16,
    pub new_fee: u16,
}