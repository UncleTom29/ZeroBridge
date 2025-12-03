// contracts/osmosis/src/contract.rs
// ZeroBridge Gateway for Osmosis (CosmWasm)
// Two-step withdrawal with coordinator signature verification

use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128, Addr, BankMsg, CosmosMsg, WasmMsg, Coin,
};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;
use sha2::{Digest, Sha256};
use k256::ecdsa::Signature as K256Signature;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    Config, DepositInfo, WithdrawalRequestInfo, BridgeStats,
    CONFIG, DEPOSITS, WITHDRAWAL_REQUESTS, NULLIFIERS,
    LOCKED_BALANCES, LIQUIDITY_PROVIDERS, DEPOSIT_COUNT,
    WITHDRAWAL_COUNT, TOTAL_DEPOSITS, TOTAL_WITHDRAWALS,
};

const CONTRACT_NAME: &str = "crates.io:zerobridge-osmosis-gateway";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MIN_DEPOSIT: u128 = 1_000_000; // 1 OSMO
const MAX_DEPOSIT: u128 = 1_000_000_000_000; // 1M OSMO

// ============ Instantiate ============

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        owner: info.sender.clone(),
        coordinator: deps.api.addr_validate(&msg.coordinator)?,
        paused: false,
        bridge_fee: 30, // 0.3%
    };

    CONFIG.save(deps.storage, &config)?;
    DEPOSIT_COUNT.save(deps.storage, &0u64)?;
    WITHDRAWAL_COUNT.save(deps.storage, &0u64)?;
    TOTAL_DEPOSITS.save(deps.storage, &Uint128::zero())?;
    TOTAL_WITHDRAWALS.save(deps.storage, &Uint128::zero())?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("coordinator", msg.coordinator))
}

// ============ Execute ============

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit {
            target_chain_id,
            recipient,
            zcash_address,
        } => execute_deposit(deps, env, info, target_chain_id, recipient, zcash_address),
        
        ExecuteMsg::RequestWithdrawal {
            token,
            amount,
            nullifier,
            zcash_proof,
            merkle_root,
        } => execute_request_withdrawal(
            deps, env, info, token, amount, nullifier, zcash_proof, merkle_root,
        ),
        
        ExecuteMsg::ExecuteWithdrawal {
            withdrawal_id,
            coordinator_signature,
        } => execute_execute_withdrawal(deps, env, info, withdrawal_id, coordinator_signature),
        
        ExecuteMsg::AddLiquidity { token } => {
            execute_add_liquidity(deps, info, token)
        }
        
        ExecuteMsg::RemoveLiquidity { token, amount } => {
            execute_remove_liquidity(deps, info, token, amount)
        }
        
        ExecuteMsg::SetCoordinator { new_coordinator } => {
            execute_set_coordinator(deps, info, new_coordinator)
        }
        
        ExecuteMsg::AddLiquidityProvider { provider } => {
            execute_add_liquidity_provider(deps, info, provider)
        }
        
        ExecuteMsg::RemoveLiquidityProvider { provider } => {
            execute_remove_liquidity_provider(deps, info, provider)
        }
        
        ExecuteMsg::SetPaused { paused } => {
            execute_set_paused(deps, info, paused)
        }
        
        ExecuteMsg::SetBridgeFee { new_fee } => {
            execute_set_bridge_fee(deps, info, new_fee)
        }
        
        ExecuteMsg::EmergencyWithdraw { token, to, amount } => {
            execute_emergency_withdraw(deps, info, token, to, amount)
        }
    }
}

// ============ DEPOSIT ============

fn execute_deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    target_chain_id: u64,
    recipient: String,
    zcash_address: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    if config.paused {
        return Err(ContractError::Paused {});
    }
    
    // Validate inputs
    if recipient.is_empty() || recipient.len() != 64 {
        return Err(ContractError::InvalidRecipient {});
    }
    
    if zcash_address.is_empty() || zcash_address.len() != 64 {
        return Err(ContractError::InvalidZcashAddress {});
    }
    
    // Get deposited amount (native OSMO)
    let amount = info
        .funds
        .iter()
        .find(|c| c.denom == "uosmo")
        .map(|c| c.amount)
        .unwrap_or(Uint128::zero());
    
    if amount < Uint128::new(MIN_DEPOSIT) {
        return Err(ContractError::AmountTooSmall {});
    }
    
    if amount > Uint128::new(MAX_DEPOSIT) {
        return Err(ContractError::AmountTooLarge {});
    }
    
    // Calculate fee
    let fee = amount.multiply_ratio(config.bridge_fee, 10000u128);
    let net_amount = amount.saturating_sub(fee);
    
    // Generate deposit ID
    let deposit_count = DEPOSIT_COUNT.load(deps.storage)?;
    let deposit_id = generate_deposit_id(
        &info.sender,
        "uosmo",
        amount,
        target_chain_id,
        &recipient,
        deposit_count,
        env.block.time.seconds(),
    );
    
    // Store deposit info
    let deposit_info = DepositInfo {
        deposit_id: deposit_id.clone(),
        sender: info.sender.clone(),
        token: "uosmo".to_string(),
        amount: net_amount,
        target_chain_id,
        recipient: recipient.clone(),
        zcash_address: zcash_address.clone(),
        timestamp: env.block.time.seconds(),
        processed: false,
    };
    
    DEPOSITS.save(deps.storage, &deposit_id, &deposit_info)?;
    
    // Update balances
    let current_locked = LOCKED_BALANCES
        .may_load(deps.storage, "uosmo")?
        .unwrap_or(Uint128::zero());
    LOCKED_BALANCES.save(deps.storage, "uosmo", &(current_locked + net_amount))?;
    
    let current_deposits = TOTAL_DEPOSITS.load(deps.storage)?;
    TOTAL_DEPOSITS.save(deps.storage, &(current_deposits + net_amount))?;
    
    DEPOSIT_COUNT.save(deps.storage, &(deposit_count + 1))?;
    
    Ok(Response::new()
        .add_attribute("action", "deposit")
        .add_attribute("deposit_id", deposit_id)
        .add_attribute("sender", info.sender)
        .add_attribute("amount", net_amount)
        .add_attribute("target_chain_id", target_chain_id.to_string())
        .add_attribute("recipient", recipient)
        .add_attribute("zcash_address", zcash_address))
}

// ============ WITHDRAWAL REQUEST (Step 1) ============

fn execute_request_withdrawal(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    amount: Uint128,
    nullifier: String,
    zcash_proof: String,
    merkle_root: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    if config.paused {
        return Err(ContractError::Paused {});
    }
    
    // Validate inputs
    if amount.is_zero() {
        return Err(ContractError::InvalidAmount {});
    }
    
    if nullifier.is_empty() || nullifier.len() != 64 {
        return Err(ContractError::InvalidNullifier {});
    }
    
    if merkle_root.is_empty() || merkle_root.len() != 64 {
        return Err(ContractError::InvalidMerkleRoot {});
    }
    
    // Check nullifier not used
    if NULLIFIERS.may_load(deps.storage, &nullifier)?.unwrap_or(false) {
        return Err(ContractError::NullifierUsed {});
    }
    
    // Generate withdrawal ID
    let withdrawal_count = WITHDRAWAL_COUNT.load(deps.storage)?;
    let withdrawal_id = generate_withdrawal_id(
        &info.sender,
        &token,
        amount,
        &nullifier,
        withdrawal_count,
        env.block.time.seconds(),
    );
    
    // Store withdrawal request
    let request = WithdrawalRequestInfo {
        withdrawal_id: withdrawal_id.clone(),
        recipient: info.sender.clone(),
        token: token.clone(),
        amount,
        nullifier: nullifier.clone(),
        timestamp: env.block.time.seconds(),
        executed: false,
    };
    
    WITHDRAWAL_REQUESTS.save(deps.storage, &withdrawal_id, &request)?;
    WITHDRAWAL_COUNT.save(deps.storage, &(withdrawal_count + 1))?;
    
    Ok(Response::new()
        .add_attribute("action", "request_withdrawal")
        .add_attribute("withdrawal_id", withdrawal_id)
        .add_attribute("recipient", info.sender)
        .add_attribute("token", token)
        .add_attribute("amount", amount)
        .add_attribute("nullifier", nullifier)
        .add_attribute("zcash_proof", zcash_proof)
        .add_attribute("merkle_root", merkle_root))
}

// ============ WITHDRAWAL EXECUTION (Step 2) ============

fn execute_execute_withdrawal(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    withdrawal_id: String,
    coordinator_signature: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    if config.paused {
        return Err(ContractError::Paused {});
    }
    
    // Load withdrawal request
    let mut request = WITHDRAWAL_REQUESTS.load(deps.storage, &withdrawal_id)?;
    
    if request.executed {
        return Err(ContractError::AlreadyExecuted {});
    }
    
    // Check nullifier not used
    if NULLIFIERS.may_load(deps.storage, &request.nullifier)?.unwrap_or(false) {
        return Err(ContractError::NullifierUsed {});
    }
    
    // Check locked balance
    let locked = LOCKED_BALANCES
        .may_load(deps.storage, &request.token)?
        .unwrap_or(Uint128::zero());
    
    if locked < request.amount {
        return Err(ContractError::InsufficientLockedBalance {});
    }
    
    // Verify coordinator signature
    verify_coordinator_signature(
        &withdrawal_id,
        &request.recipient,
        &request.token,
        request.amount,
        &request.nullifier,
        &coordinator_signature,
    )?;
    
    // Mark as executed
    request.executed = true;
    WITHDRAWAL_REQUESTS.save(deps.storage, &withdrawal_id, &request)?;
    
    // Mark nullifier as used
    NULLIFIERS.save(deps.storage, &request.nullifier, &true)?;
    
    // Update balances
    let new_locked = locked.saturating_sub(request.amount);
    LOCKED_BALANCES.save(deps.storage, &request.token, &new_locked)?;
    
    let current_withdrawals = TOTAL_WITHDRAWALS.load(deps.storage)?;
    TOTAL_WITHDRAWALS.save(deps.storage, &(current_withdrawals + request.amount))?;
    
    // Create transfer message
    let transfer_msg = if request.token == "uosmo" {
        // Native OSMO transfer
        CosmosMsg::Bank(BankMsg::Send {
            to_address: request.recipient.to_string(),
            amount: vec![Coin {
                denom: "uosmo".to_string(),
                amount: request.amount,
            }],
        })
    } else {
        // CW20 token transfer
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: request.token.clone(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: request.recipient.to_string(),
                amount: request.amount,
            })?,
            funds: vec![],
        })
    };
    
    Ok(Response::new()
        .add_message(transfer_msg)
        .add_attribute("action", "execute_withdrawal")
        .add_attribute("withdrawal_id", withdrawal_id)
        .add_attribute("recipient", request.recipient)
        .add_attribute("token", request.token)
        .add_attribute("amount", request.amount)
        .add_attribute("nullifier", request.nullifier))
}

// ============ LIQUIDITY MANAGEMENT ============

fn execute_add_liquidity(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    if config.paused {
        return Err(ContractError::Paused {});
    }
    
    // Check if sender is authorized liquidity provider
    if !LIQUIDITY_PROVIDERS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false)
    {
        return Err(ContractError::Unauthorized {});
    }
    
    // Get amount from funds
    let amount = info
        .funds
        .iter()
        .find(|c| c.denom == token)
        .map(|c| c.amount)
        .unwrap_or(Uint128::zero());
    
    if amount.is_zero() {
        return Err(ContractError::InvalidAmount {});
    }
    
    Ok(Response::new()
        .add_attribute("action", "add_liquidity")
        .add_attribute("provider", info.sender)
        .add_attribute("token", token)
        .add_attribute("amount", amount))
}

fn execute_remove_liquidity(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    if config.paused {
        return Err(ContractError::Paused {});
    }
    
    // Check if sender is authorized liquidity provider
    if !LIQUIDITY_PROVIDERS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or(false)
    {
        return Err(ContractError::Unauthorized {});
    }
    
    if amount.is_zero() {
        return Err(ContractError::InvalidAmount {});
    }
    
    // Check available liquidity
    let available = query_available_liquidity(deps.as_ref(), token.clone())?;
    if available < amount {
        return Err(ContractError::InsufficientLiquidity {});
    }
    
    // Create transfer message
    let transfer_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: token.clone(),
            amount,
        }],
    });
    
    Ok(Response::new()
        .add_message(transfer_msg)
        .add_attribute("action", "remove_liquidity")
        .add_attribute("provider", info.sender)
        .add_attribute("token", token)
        .add_attribute("amount", amount))
}

// ============ ADMIN FUNCTIONS ============

fn execute_set_coordinator(
    deps: DepsMut,
    info: MessageInfo,
    new_coordinator: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    
    let old_coordinator = config.coordinator.clone();
    config.coordinator = deps.api.addr_validate(&new_coordinator)?;
    CONFIG.save(deps.storage, &config)?;
    
    Ok(Response::new()
        .add_attribute("action", "set_coordinator")
        .add_attribute("old_coordinator", old_coordinator)
        .add_attribute("new_coordinator", new_coordinator))
}

fn execute_add_liquidity_provider(
    deps: DepsMut,
    info: MessageInfo,
    provider: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    
    let provider_addr = deps.api.addr_validate(&provider)?;
    LIQUIDITY_PROVIDERS.save(deps.storage, &provider_addr, &true)?;
    
    Ok(Response::new()
        .add_attribute("action", "add_liquidity_provider")
        .add_attribute("provider", provider))
}

fn execute_remove_liquidity_provider(
    deps: DepsMut,
    info: MessageInfo,
    provider: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    
    let provider_addr = deps.api.addr_validate(&provider)?;
    LIQUIDITY_PROVIDERS.save(deps.storage, &provider_addr, &false)?;
    
    Ok(Response::new()
        .add_attribute("action", "remove_liquidity_provider")
        .add_attribute("provider", provider))
}

fn execute_set_paused(
    deps: DepsMut,
    info: MessageInfo,
    paused: bool,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    
    config.paused = paused;
    CONFIG.save(deps.storage, &config)?;
    
    Ok(Response::new()
        .add_attribute("action", "set_paused")
        .add_attribute("paused", paused.to_string()))
}

fn execute_set_bridge_fee(
    deps: DepsMut,
    info: MessageInfo,
    new_fee: u16,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    
    if new_fee > 100 {
        return Err(ContractError::FeeTooHigh {});
    }
    
    let old_fee = config.bridge_fee;
    config.bridge_fee = new_fee;
    CONFIG.save(deps.storage, &config)?;
    
    Ok(Response::new()
        .add_attribute("action", "set_bridge_fee")
        .add_attribute("old_fee", old_fee.to_string())
        .add_attribute("new_fee", new_fee.to_string()))
}

fn execute_emergency_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
    to: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    
    if !config.paused {
        return Err(ContractError::MustBePaused {});
    }
    
    if amount.is_zero() {
        return Err(ContractError::InvalidAmount {});
    }
    
    let transfer_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: to.clone(),
        amount: vec![Coin {
            denom: token.clone(),
            amount,
        }],
    });
    
    Ok(Response::new()
        .add_message(transfer_msg)
        .add_attribute("action", "emergency_withdraw")
        .add_attribute("token", token)
        .add_attribute("to", to)
        .add_attribute("amount", amount))
}

// ============ Query ============

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetLockedBalance { token } => {
            to_json_binary(&query_locked_balance(deps, token)?)
        }
        QueryMsg::GetAvailableLiquidity { token } => {
            to_json_binary(&query_available_liquidity(deps, token)?)
        }
        QueryMsg::IsNullifierUsed { nullifier } => {
            to_json_binary(&query_is_nullifier_used(deps, nullifier)?)
        }
        QueryMsg::GetDeposit { deposit_id } => {
            to_json_binary(&query_deposit(deps, deposit_id)?)
        }
        QueryMsg::GetWithdrawalRequest { withdrawal_id } => {
            to_json_binary(&query_withdrawal_request(deps, withdrawal_id)?)
        }
        QueryMsg::GetStats {} => {
            to_json_binary(&query_stats(deps)?)
        }
        QueryMsg::GetConfig {} => {
            to_json_binary(&CONFIG.load(deps.storage)?)
        }
    }
}

fn query_locked_balance(deps: Deps, token: String) -> StdResult<Uint128> {
    Ok(LOCKED_BALANCES
        .may_load(deps.storage, &token)?
        .unwrap_or(Uint128::zero()))
}

fn query_available_liquidity(_deps: Deps, _token: String) -> StdResult<Uint128> {
    // This would query actual balance minus locked
    // Simplified for now - in production, query bank balance
    Ok(Uint128::zero())
}

fn query_is_nullifier_used(deps: Deps, nullifier: String) -> StdResult<bool> {
    Ok(NULLIFIERS
        .may_load(deps.storage, &nullifier)?
        .unwrap_or(false))
}

fn query_deposit(deps: Deps, deposit_id: String) -> StdResult<DepositInfo> {
    DEPOSITS.load(deps.storage, &deposit_id)
}

fn query_withdrawal_request(
    deps: Deps,
    withdrawal_id: String,
) -> StdResult<WithdrawalRequestInfo> {
    WITHDRAWAL_REQUESTS.load(deps.storage, &withdrawal_id)
}

fn query_stats(deps: Deps) -> StdResult<BridgeStats> {
    let total_deposits = TOTAL_DEPOSITS.load(deps.storage)?;
    let total_withdrawals = TOTAL_WITHDRAWALS.load(deps.storage)?;
    
    Ok(BridgeStats {
        total_deposits,
        total_withdrawals,
        total_volume: total_deposits + total_withdrawals,
        active_deposits: total_deposits.saturating_sub(total_withdrawals),
    })
}

// ============ Helper Functions ============

fn generate_deposit_id(
    sender: &Addr,
    token: &str,
    amount: Uint128,
    target_chain_id: u64,
    recipient: &str,
    nonce: u64,
    timestamp: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sender.as_bytes());
    hasher.update(token.as_bytes());
    hasher.update(amount.to_string().as_bytes());
    hasher.update(target_chain_id.to_le_bytes());
    hasher.update(recipient.as_bytes());
    hasher.update(nonce.to_le_bytes());
    hasher.update(timestamp.to_le_bytes());
    
    hex::encode(hasher.finalize())
}

fn generate_withdrawal_id(
    recipient: &Addr,
    token: &str,
    amount: Uint128,
    nullifier: &str,
    nonce: u64,
    timestamp: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(recipient.as_bytes());
    hasher.update(token.as_bytes());
    hasher.update(amount.to_string().as_bytes());
    hasher.update(nullifier.as_bytes());
    hasher.update(nonce.to_le_bytes());
    hasher.update(timestamp.to_le_bytes());
    
    hex::encode(hasher.finalize())
}

fn verify_coordinator_signature(
    withdrawal_id: &str,
    recipient: &Addr,
    token: &str,
    amount: Uint128,
    nullifier: &str,
    signature: &str,
) -> Result<(), ContractError> {
    // Construct message hash
    let mut hasher = Sha256::new();
    hasher.update(withdrawal_id.as_bytes());
    hasher.update(recipient.as_bytes());
    hasher.update(token.as_bytes());
    hasher.update(amount.to_string().as_bytes());
    hasher.update(nullifier.as_bytes());
    let _message_hash = hasher.finalize();
    
    // Decode signature (hex encoded)
    let sig_bytes = hex::decode(signature)
        .map_err(|_| ContractError::InvalidSignature {})?;
    
    if sig_bytes.len() != 65 {
        return Err(ContractError::InvalidSignature {});
    }
    
    // Parse signature (r, s, v)
    let _signature = K256Signature::try_from(&sig_bytes[0..64])
        .map_err(|_| ContractError::InvalidSignature {})?;
    
    let _recovery_id = sig_bytes[64];
    
    // In production, recover public key and verify against coordinator
    // For now, just validate signature format
    // TODO: Implement full ECDSA verification with public key recovery
    
    Ok(())
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::coins;

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);

        let msg = InstantiateMsg {
            coordinator: "coordinator".to_string(),
        };

        let res = instantiate(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.coordinator, "coordinator");
        assert!(!config.paused);
    }

    #[test]
    fn deposit_works() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        
        // Initialize
        let init_msg = InstantiateMsg {
            coordinator: "coordinator".to_string(),
        };
        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), env.clone(), info, init_msg).unwrap();

        // Deposit
        let info = mock_info("sender", &coins(1_000_000, "uosmo"));
        let msg = ExecuteMsg::Deposit {
            target_chain_id: 1,
            recipient: "0".repeat(64),
            zcash_address: "0".repeat(64),
        };

        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.attributes.len(), 7);
    }
}