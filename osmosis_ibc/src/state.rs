// ============================================
// contracts/osmosis/src/state.rs
// State definitions

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub coordinator: Addr,
    pub paused: bool,
    pub bridge_fee: u16,
}

#[cw_serde]
pub struct DepositInfo {
    pub deposit_id: String,
    pub sender: Addr,
    pub token: String,
    pub amount: Uint128,
    pub target_chain_id: u64,
    pub recipient: String,
    pub zcash_address: String,
    pub timestamp: u64,
    pub processed: bool,
}

#[cw_serde]
pub struct WithdrawalRequestInfo {
    pub withdrawal_id: String,
    pub recipient: Addr,
    pub token: String,
    pub amount: Uint128,
    pub nullifier: String,
    pub timestamp: u64,
    pub executed: bool,
}

#[cw_serde]
pub struct BridgeStats {
    pub total_deposits: Uint128,
    pub total_withdrawals: Uint128,
    pub total_volume: Uint128,
    pub active_deposits: Uint128,
}

// Storage
pub const CONFIG: Item<Config> = Item::new("config");
pub const DEPOSITS: Map<&str, DepositInfo> = Map::new("deposits");
pub const WITHDRAWAL_REQUESTS: Map<&str, WithdrawalRequestInfo> = Map::new("withdrawal_requests");
pub const NULLIFIERS: Map<&str, bool> = Map::new("nullifiers");
pub const LOCKED_BALANCES: Map<&str, Uint128> = Map::new("locked_balances");
pub const LIQUIDITY_PROVIDERS: Map<&Addr, bool> = Map::new("liquidity_providers");
pub const DEPOSIT_COUNT: Item<u64> = Item::new("deposit_count");
pub const WITHDRAWAL_COUNT: Item<u64> = Item::new("withdrawal_count");
pub const TOTAL_DEPOSITS: Item<Uint128> = Item::new("total_deposits");
pub const TOTAL_WITHDRAWALS: Item<Uint128> = Item::new("total_withdrawals");
