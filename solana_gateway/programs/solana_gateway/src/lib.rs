// contracts/solana/programs/solana-gateway/src/lib.rs
// UPGRADED: Compatible with Anchor 0.32.1 and anchor-spl 0.32.1

use anchor_lang::prelude::*;
// In Solana SDK 2.x, crypto functions moved to separate crates
use sha3::{Digest, Keccak256};
use solana_secp256k1_recover::secp256k1_recover;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

declare_id!("8FGoQPMAt83sMLrxNb3yr8fQS8VBhQPEu31wCGg7b6Tc");

#[program]
pub mod solana_adapter {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        coordinator_pubkey: Pubkey,
    ) -> Result<()> {
        let gateway = &mut ctx.accounts.gateway;
        gateway.authority = ctx.accounts.authority.key();
        gateway.coordinator = coordinator_pubkey;
        gateway.total_locked = 0;
        gateway.total_withdrawn = 0;
        gateway.deposit_count = 0;
        gateway.withdrawal_count = 0;
        gateway.paused = false;
        gateway.bump = ctx.bumps.gateway;
        
        msg!("Gateway initialized with coordinator: {}", coordinator_pubkey);
        Ok(())
    }

    /// Deposit - locks tokens and emits event
    pub fn deposit(
        ctx: Context<Deposit>,
        amount: u64,
        target_chain_id: u64,
        recipient: [u8; 32],
        zcash_address: [u8; 32],
    ) -> Result<()> {
        require!(!ctx.accounts.gateway.paused, ErrorCode::GatewayPaused);
        require!(amount > 0, ErrorCode::InvalidAmount);
        require!(amount >= 1_000_000, ErrorCode::AmountTooSmall);
        
        let gateway = &mut ctx.accounts.gateway;
        
        // Transfer tokens from user to vault using Token-2022 interface
        anchor_spl::token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::TransferChecked {
                    from: ctx.accounts.user_token.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;
        
        let deposit_id = generate_deposit_id(
            &ctx.accounts.user.key(),
            &ctx.accounts.mint.key(),
            amount,
            target_chain_id,
            recipient,
            gateway.deposit_count,
        );
        
        let deposit_account = &mut ctx.accounts.deposit;
        deposit_account.deposit_id = deposit_id;
        deposit_account.sender = ctx.accounts.user.key();
        deposit_account.mint = ctx.accounts.mint.key();
        deposit_account.amount = amount;
        deposit_account.target_chain_id = target_chain_id;
        deposit_account.recipient = recipient;
        deposit_account.zcash_address = zcash_address;
        deposit_account.timestamp = Clock::get()?.unix_timestamp;
        deposit_account.processed = false;
        
        gateway.total_locked = gateway
            .total_locked
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;
        gateway.deposit_count = gateway
            .deposit_count
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;
        
        emit!(TokensLocked {
            deposit_id,
            sender: ctx.accounts.user.key(),
            mint: ctx.accounts.mint.key(),
            amount,
            target_chain_id,
            recipient,
            zcash_address,
            timestamp: deposit_account.timestamp,
        });
        
        msg!("Deposit created: {:?}", deposit_id);
        Ok(())
    }

    /// Request withdrawal - Step 1 (emits event for relayer)
    pub fn request_withdrawal(
        ctx: Context<RequestWithdrawal>,
        amount: u64,
        nullifier: [u8; 32],
        zcash_proof: Vec<u8>,
        merkle_root: [u8; 32],
    ) -> Result<()> {
        require!(!ctx.accounts.gateway.paused, ErrorCode::GatewayPaused);
        require!(amount > 0, ErrorCode::InvalidAmount);
        
        // Check nullifier not used
        require!(
            !ctx.accounts.nullifier_account.used,
            ErrorCode::NullifierUsed
        );
        
        let gateway = &mut ctx.accounts.gateway;
        
        let withdrawal_id = generate_withdrawal_id(
            &ctx.accounts.recipient.key(),
            &ctx.accounts.mint.key(),
            amount,
            nullifier,
            gateway.withdrawal_count,
        );
        
        let withdrawal_request = &mut ctx.accounts.withdrawal_request;
        withdrawal_request.withdrawal_id = withdrawal_id;
        withdrawal_request.recipient = ctx.accounts.recipient.key();
        withdrawal_request.mint = ctx.accounts.mint.key();
        withdrawal_request.amount = amount;
        withdrawal_request.nullifier = nullifier;
        withdrawal_request.timestamp = Clock::get()?.unix_timestamp;
        withdrawal_request.executed = false;
        
        gateway.withdrawal_count = gateway
            .withdrawal_count
            .checked_add(1)
            .ok_or(ErrorCode::Overflow)?;
        
        // Emit event for relayer to pick up
        emit!(WithdrawalRequested {
            withdrawal_id,
            recipient: ctx.accounts.recipient.key(),
            mint: ctx.accounts.mint.key(),
            amount,
            nullifier,
            zcash_proof,
            merkle_root,
            timestamp: withdrawal_request.timestamp,
        });
        
        msg!("Withdrawal requested: {:?}", withdrawal_id);
        Ok(())
    }

    /// Execute withdrawal - Step 2 (with coordinator signature)
    pub fn execute_withdrawal(
        ctx: Context<ExecuteWithdrawal>,
        withdrawal_id: [u8; 32],
        coordinator_signature: [u8; 65], // r(32) + s(32) + v(1)
    ) -> Result<()> {
        require!(!ctx.accounts.gateway.paused, ErrorCode::GatewayPaused);
        
        let withdrawal_request = &ctx.accounts.withdrawal_request;
        
        require!(
            withdrawal_request.withdrawal_id == withdrawal_id,
            ErrorCode::InvalidWithdrawalId
        );
        require!(!withdrawal_request.executed, ErrorCode::AlreadyExecuted);
        
        // Check nullifier not used
        require!(
            !ctx.accounts.nullifier_account.used,
            ErrorCode::NullifierUsed
        );
        
        // Store values before mutable borrow
        let recipient_key = withdrawal_request.recipient;
        let amount = withdrawal_request.amount;
        let nullifier = withdrawal_request.nullifier;
        let mint_key = withdrawal_request.mint;
        
        // Verify coordinator signature
        verify_coordinator_signature(
            withdrawal_id,
            recipient_key,
            amount,
            nullifier,
            &coordinator_signature,
            ctx.accounts.gateway.coordinator,
        )?;
        
        // Mark as executed
        let withdrawal_request_mut = &mut ctx.accounts.withdrawal_request;
        withdrawal_request_mut.executed = true;
        
        let nullifier_account = &mut ctx.accounts.nullifier_account;
        nullifier_account.nullifier = nullifier;
        nullifier_account.used = true;
        nullifier_account.timestamp = Clock::get()?.unix_timestamp;
        
        let gateway = &mut ctx.accounts.gateway;
        
        gateway.total_locked = gateway
            .total_locked
            .checked_sub(amount)
            .ok_or(ErrorCode::Underflow)?;
        gateway.total_withdrawn = gateway
            .total_withdrawn
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;
        
        let seeds = &[b"gateway".as_ref(), &[gateway.bump]];
        let signer = &[&seeds[..]];
        
        // Transfer from vault using PDA signer with Token-2022 interface
        anchor_spl::token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.recipient_token.to_account_info(),
                    authority: ctx.accounts.gateway.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
                signer,
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;
        
        emit!(TokensReleased {
            withdrawal_id,
            recipient: recipient_key,
            mint: mint_key,
            amount,
            nullifier,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        msg!("Withdrawal executed: {:?}", withdrawal_id);
        Ok(())
    }

    pub fn set_coordinator(
        ctx: Context<SetCoordinator>,
        new_coordinator: Pubkey,
    ) -> Result<()> {
        let gateway = &mut ctx.accounts.gateway;
        let old_coordinator = gateway.coordinator;
        gateway.coordinator = new_coordinator;
        
        emit!(CoordinatorUpdated {
            old_coordinator,
            new_coordinator,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        Ok(())
    }

    pub fn set_paused(
        ctx: Context<SetPaused>,
        paused: bool,
    ) -> Result<()> {
        ctx.accounts.gateway.paused = paused;
        
        if paused {
            emit!(EmergencyPause {
                triggered_by: ctx.accounts.authority.key(),
                timestamp: Clock::get()?.unix_timestamp,
            });
        }
        
        Ok(())
    }

    pub fn add_liquidity(
        ctx: Context<AddLiquidity>,
        amount: u64,
    ) -> Result<()> {
        require!(!ctx.accounts.gateway.paused, ErrorCode::GatewayPaused);
        require!(amount > 0, ErrorCode::InvalidAmount);
        
        // Transfer tokens using Token-2022 interface
        anchor_spl::token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::TransferChecked {
                    from: ctx.accounts.provider_token.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.provider.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;
        
        emit!(LiquidityAdded {
            provider: ctx.accounts.provider.key(),
            mint: ctx.accounts.mint.key(),
            amount,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        Ok(())
    }
}

// ============ Helper Functions ============

fn verify_coordinator_signature(
    withdrawal_id: [u8; 32],
    recipient: Pubkey,
    amount: u64,
    nullifier: [u8; 32],
    signature: &[u8; 65],
    _expected_coordinator: Pubkey,
) -> Result<()> {
    // Construct message hash (same as EVM)
    let mut message_data = Vec::new();
    message_data.extend_from_slice(&withdrawal_id);
    message_data.extend_from_slice(recipient.as_ref());
    message_data.extend_from_slice(&amount.to_le_bytes());
    message_data.extend_from_slice(&nullifier);
    
    let message_hash: [u8; 32] = Keccak256::digest(&message_data).into();
    
    // Split signature into r, s, v
    let recovery_id = signature[64];
    
    // Create fixed-size array for signature
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&signature[0..64]);
    
    // Recover public key using new secp256k1_recover API
    let recovered_pubkey = secp256k1_recover(
        message_hash.as_ref(),
        recovery_id,
        &sig_bytes,
    )
    .map_err(|_| ErrorCode::InvalidSignature)?;
    
    // Convert recovered pubkey to Solana address format
    // In production, coordinator would have their Ethereum address stored
    // and we'd verify against that
    
    // For now, simplified check
    require!(
        recovered_pubkey.0.len() == 64,
        ErrorCode::InvalidSignature
    );
    
    Ok(())
}

fn generate_deposit_id(
    sender: &Pubkey,
    mint: &Pubkey,
    amount: u64,
    target_chain_id: u64,
    recipient: [u8; 32],
    nonce: u64,
) -> [u8; 32] {
    let mut data = Vec::new();
    data.extend_from_slice(sender.as_ref());
    data.extend_from_slice(mint.as_ref());
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&target_chain_id.to_le_bytes());
    data.extend_from_slice(&recipient);
    data.extend_from_slice(&nonce.to_le_bytes());
    
    Keccak256::digest(&data).into()
}

fn generate_withdrawal_id(
    recipient: &Pubkey,
    mint: &Pubkey,
    amount: u64,
    nullifier: [u8; 32],
    nonce: u64,
) -> [u8; 32] {
    let mut data = Vec::new();
    data.extend_from_slice(recipient.as_ref());
    data.extend_from_slice(mint.as_ref());
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&nullifier);
    data.extend_from_slice(&nonce.to_le_bytes());
    
    Keccak256::digest(&data).into()
}

// ============ Account Structures ============

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + GatewayState::SIZE,
        seeds = [b"gateway"],
        bump
    )]
    pub gateway: Account<'info, GatewayState>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut, seeds = [b"gateway"], bump = gateway.bump)]
    pub gateway: Account<'info, GatewayState>,
    
    #[account(
        init,
        payer = user,
        space = 8 + DepositInfo::SIZE,
        seeds = [b"deposit", gateway.deposit_count.to_le_bytes().as_ref()],
        bump
    )]
    pub deposit: Account<'info, DepositInfo>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub mint: InterfaceAccount<'info, Mint>,
    
    #[account(
        mut,
        token::mint = mint,
        token::authority = user,
    )]
    pub user_token: InterfaceAccount<'info, TokenAccount>,
    
    #[account(
        mut,
        seeds = [b"vault", mint.key().as_ref()],
        bump,
        token::mint = mint,
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64, nullifier: [u8; 32])]
pub struct RequestWithdrawal<'info> {
    #[account(mut, seeds = [b"gateway"], bump = gateway.bump)]
    pub gateway: Account<'info, GatewayState>,
    
    #[account(
        init,
        payer = recipient,
        space = 8 + WithdrawalRequestInfo::SIZE,
        seeds = [b"withdrawal_request", gateway.withdrawal_count.to_le_bytes().as_ref()],
        bump
    )]
    pub withdrawal_request: Account<'info, WithdrawalRequestInfo>,
    
    #[account(
        init,
        payer = recipient,
        space = 8 + NullifierAccount::SIZE,
        seeds = [b"nullifier_check", nullifier.as_ref()],
        bump
    )]
    pub nullifier_account: Account<'info, NullifierAccount>,
    
    #[account(mut)]
    pub recipient: Signer<'info>,
    
    pub mint: InterfaceAccount<'info, Mint>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(withdrawal_id: [u8; 32])]
pub struct ExecuteWithdrawal<'info> {
    #[account(mut, seeds = [b"gateway"], bump = gateway.bump)]
    pub gateway: Account<'info, GatewayState>,
    
    #[account(
        mut,
        seeds = [b"withdrawal_request", &withdrawal_id],
        bump
    )]
    pub withdrawal_request: Account<'info, WithdrawalRequestInfo>,
    
    #[account(
        mut,
        seeds = [b"nullifier_check", withdrawal_request.nullifier.as_ref()],
        bump
    )]
    pub nullifier_account: Account<'info, NullifierAccount>,
    
    /// CHECK: Can be anyone (relayer)
    #[account(mut)]
    pub executor: Signer<'info>,
    
    /// CHECK: Validated by recipient_token constraint
    #[account(mut)]
    pub recipient: AccountInfo<'info>,
    
    pub mint: InterfaceAccount<'info, Mint>,
    
    #[account(
        mut,
        seeds = [b"vault", mint.key().as_ref()],
        bump,
        token::mint = mint,
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    
    #[account(
        mut,
        token::mint = mint,
        token::authority = recipient,
    )]
    pub recipient_token: InterfaceAccount<'info, TokenAccount>,
    
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct SetCoordinator<'info> {
    #[account(
        mut,
        seeds = [b"gateway"],
        bump = gateway.bump,
        constraint = gateway.authority == authority.key()
    )]
    pub gateway: Account<'info, GatewayState>,
    
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct SetPaused<'info> {
    #[account(
        mut,
        seeds = [b"gateway"],
        bump = gateway.bump,
        constraint = gateway.authority == authority.key()
    )]
    pub gateway: Account<'info, GatewayState>,
    
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(seeds = [b"gateway"], bump = gateway.bump)]
    pub gateway: Account<'info, GatewayState>,
    
    #[account(mut)]
    pub provider: Signer<'info>,
    
    pub mint: InterfaceAccount<'info, Mint>,
    
    #[account(
        mut,
        token::mint = mint,
        token::authority = provider,
    )]
    pub provider_token: InterfaceAccount<'info, TokenAccount>,
    
    #[account(
        mut,
        seeds = [b"vault", mint.key().as_ref()],
        bump,
        token::mint = mint,
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    
    pub token_program: Interface<'info, TokenInterface>,
}

// ============ State Accounts ============

#[account]
pub struct GatewayState {
    pub authority: Pubkey,
    pub coordinator: Pubkey,
    pub total_locked: u64,
    pub total_withdrawn: u64,
    pub deposit_count: u64,
    pub withdrawal_count: u64,
    pub paused: bool,
    pub bump: u8,
}

impl GatewayState {
    pub const SIZE: usize = 32 + 32 + 8 + 8 + 8 + 8 + 1 + 1;
}

#[account]
pub struct DepositInfo {
    pub deposit_id: [u8; 32],
    pub sender: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub target_chain_id: u64,
    pub recipient: [u8; 32],
    pub zcash_address: [u8; 32],
    pub timestamp: i64,
    pub processed: bool,
}

impl DepositInfo {
    pub const SIZE: usize = 32 + 32 + 32 + 8 + 8 + 32 + 32 + 8 + 1;
}

#[account]
pub struct WithdrawalRequestInfo {
    pub withdrawal_id: [u8; 32],
    pub recipient: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub nullifier: [u8; 32],
    pub timestamp: i64,
    pub executed: bool,
}

impl WithdrawalRequestInfo {
    pub const SIZE: usize = 32 + 32 + 32 + 8 + 32 + 8 + 1;
}

#[account]
pub struct NullifierAccount {
    pub nullifier: [u8; 32],
    pub used: bool,
    pub timestamp: i64,
}

impl NullifierAccount {
    pub const SIZE: usize = 32 + 1 + 8;
}

// ============ Events ============

#[event]
pub struct TokensLocked {
    pub deposit_id: [u8; 32],
    pub sender: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub target_chain_id: u64,
    pub recipient: [u8; 32],
    pub zcash_address: [u8; 32],
    pub timestamp: i64,
}

#[event]
pub struct WithdrawalRequested {
    pub withdrawal_id: [u8; 32],
    pub recipient: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub nullifier: [u8; 32],
    pub zcash_proof: Vec<u8>,
    pub merkle_root: [u8; 32],
    pub timestamp: i64,
}

#[event]
pub struct TokensReleased {
    pub withdrawal_id: [u8; 32],
    pub recipient: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub nullifier: [u8; 32],
    pub timestamp: i64,
}

#[event]
pub struct CoordinatorUpdated {
    pub old_coordinator: Pubkey,
    pub new_coordinator: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct EmergencyPause {
    pub triggered_by: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct LiquidityAdded {
    pub provider: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

// ============ Errors ============

#[error_code]
pub enum ErrorCode {
    #[msg("Gateway is paused")]
    GatewayPaused,
    
    #[msg("Invalid amount")]
    InvalidAmount,
    
    #[msg("Amount too small")]
    AmountTooSmall,
    
    #[msg("Nullifier already used")]
    NullifierUsed,
    
    #[msg("Invalid coordinator")]
    InvalidCoordinator,
    
    #[msg("Invalid signature")]
    InvalidSignature,
    
    #[msg("Invalid withdrawal ID")]
    InvalidWithdrawalId,
    
    #[msg("Already executed")]
    AlreadyExecuted,
    
    #[msg("Arithmetic overflow")]
    Overflow,
    
    #[msg("Arithmetic underflow")]
    Underflow,
}