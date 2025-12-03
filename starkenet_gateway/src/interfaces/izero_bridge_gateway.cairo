use starknet::ContractAddress;
use crate::types::{DepositInfo, WithdrawalRequestInfo, BridgeStats};

#[starknet::interface]
pub trait IZeroBridgeGateway<TContractState> {
    // User functions
    fn deposit(
        ref self: TContractState,
        token: ContractAddress,
        amount: u256,
        target_chain_id: u64,
        recipient: felt252,
        zcash_address: felt252,
    ) -> felt252;

    fn request_withdrawal(
        ref self: TContractState,
        token: ContractAddress,
        amount: u256,
        nullifier: felt252,
        zcash_proof: Array<felt252>,
        merkle_root: felt252,
    ) -> felt252;

    fn execute_withdrawal(
        ref self: TContractState,
        withdrawal_id: felt252,
        coordinator_signature_r: felt252,
        coordinator_signature_s: felt252,
    ) -> bool;

    // Liquidity
    fn add_liquidity(ref self: TContractState, token: ContractAddress, amount: u256);
    fn remove_liquidity(ref self: TContractState, token: ContractAddress, amount: u256);

    // View
    fn get_locked_balance(self: @TContractState, token: ContractAddress) -> u256;
    fn get_available_liquidity(self: @TContractState, token: ContractAddress) -> u256;
    fn is_nullifier_used(self: @TContractState, nullifier: felt252) -> bool;
    fn get_deposit(self: @TContractState, deposit_id: felt252) -> DepositInfo;
    fn get_withdrawal_request(self: @TContractState, withdrawal_id: felt252) -> WithdrawalRequestInfo;
    fn get_stats(self: @TContractState) -> BridgeStats;

    // Admin
    fn set_coordinator(ref self: TContractState, new_coordinator: ContractAddress);
    fn add_liquidity_provider(ref self: TContractState, provider: ContractAddress);
    fn remove_liquidity_provider(ref self: TContractState, provider: ContractAddress);
    fn set_paused(ref self: TContractState, paused: bool);
    fn set_bridge_fee(ref self: TContractState, new_fee: u16);
    fn emergency_withdraw(ref self: TContractState, token: ContractAddress, to: ContractAddress, amount: u256);
}