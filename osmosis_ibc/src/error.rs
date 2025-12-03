// ============================================
// contracts/osmosis/src/error.rs
// Error definitions

use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Gateway is paused")]
    Paused {},

    #[error("Must be paused")]
    MustBePaused {},

    #[error("Amount too small")]
    AmountTooSmall {},

    #[error("Amount too large")]
    AmountTooLarge {},

    #[error("Invalid amount")]
    InvalidAmount {},

    #[error("Invalid recipient")]
    InvalidRecipient {},

    #[error("Invalid Zcash address")]
    InvalidZcashAddress {},

    #[error("Invalid nullifier")]
    InvalidNullifier {},

    #[error("Invalid merkle root")]
    InvalidMerkleRoot {},

    #[error("Nullifier already used")]
    NullifierUsed {},

    #[error("Already executed")]
    AlreadyExecuted {},

    #[error("Insufficient locked balance")]
    InsufficientLockedBalance {},

    #[error("Insufficient liquidity")]
    InsufficientLiquidity {},

    #[error("Invalid signature")]
    InvalidSignature {},

    #[error("Fee too high")]
    FeeTooHigh {},
}
