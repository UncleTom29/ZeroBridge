// ============================================
// contracts/osmosis/src/msg.rs
// Message definitions

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub coordinator: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    Deposit {
        target_chain_id: u64,
        recipient: String,
        zcash_address: String,
    },
    RequestWithdrawal {
        token: String,
        amount: Uint128,
        nullifier: String,
        zcash_proof: String,
        merkle_root: String,
    },
    ExecuteWithdrawal {
        withdrawal_id: String,
        coordinator_signature: String,
    },
    AddLiquidity {
        token: String,
    },
    RemoveLiquidity {
        token: String,
        amount: Uint128,
    },
    SetCoordinator {
        new_coordinator: String,
    },
    AddLiquidityProvider {
        provider: String,
    },
    RemoveLiquidityProvider {
        provider: String,
    },
    SetPaused {
        paused: bool,
    },
    SetBridgeFee {
        new_fee: u16,
    },
    EmergencyWithdraw {
        token: String,
        to: String,
        amount: Uint128,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Uint128)]
    GetLockedBalance { token: String },
    
    #[returns(Uint128)]
    GetAvailableLiquidity { token: String },
    
    #[returns(bool)]
    IsNullifierUsed { nullifier: String },
    
    #[returns(crate::state::DepositInfo)]
    GetDeposit { deposit_id: String },
    
    #[returns(crate::state::WithdrawalRequestInfo)]
    GetWithdrawalRequest { withdrawal_id: String },
    
    #[returns(crate::state::BridgeStats)]
    GetStats {},
    
    #[returns(crate::state::Config)]
    GetConfig {},
}