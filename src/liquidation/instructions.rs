use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::utils::get_program_id;

/// Create refresh reserve instruction
/// Equivalent to models/instructions/refreshReserve.ts
pub fn refresh_reserve_instruction(
    env: &str,
    reserve: &Pubkey,
    pyth_oracle: &Pubkey,
    switchboard_oracle: &Pubkey,
) -> Result<Instruction> {
    let program_id = get_program_id(env)?;
    
    // Instruction discriminator for RefreshReserve (instruction index 3)
    let mut data = vec![3];
    
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(*reserve, false),
            AccountMeta::new_readonly(*pyth_oracle, false),
            AccountMeta::new_readonly(*switchboard_oracle, false),
        ],
        data,
    })
}

/// Create refresh obligation instruction
/// Equivalent to models/instructions/refreshObligation.ts
pub fn refresh_obligation_instruction(
    env: &str,
    obligation: &Pubkey,
    deposit_reserves: &[Pubkey],
    borrow_reserves: &[Pubkey],
) -> Result<Instruction> {
    let program_id = get_program_id(env)?;
    
    // Instruction discriminator for RefreshObligation (instruction index 7)
    let mut data = vec![7];
    
    let mut accounts = vec![
        AccountMeta::new(*obligation, false),
        AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
    ];
    
    // Add deposit reserves
    for reserve in deposit_reserves {
        accounts.push(AccountMeta::new_readonly(*reserve, false));
    }
    
    // Add borrow reserves
    for reserve in borrow_reserves {
        accounts.push(AccountMeta::new_readonly(*reserve, false));
    }
    
    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Create liquidate obligation and redeem reserve collateral instruction
/// Equivalent to models/instructions/LiquidateObligationAndRedeemReserveCollateral.ts
#[allow(clippy::too_many_arguments)]
pub fn liquidate_and_redeem_instruction(
    env: &str,
    liquidity_amount: u64,
    repay_account: &Pubkey,
    withdraw_collateral_account: &Pubkey,
    withdraw_liquidity_account: &Pubkey,
    repay_reserve: &Pubkey,
    repay_reserve_liquidity: &Pubkey,
    withdraw_reserve: &Pubkey,
    withdraw_reserve_collateral_mint: &Pubkey,
    withdraw_reserve_collateral_supply: &Pubkey,
    withdraw_reserve_liquidity: &Pubkey,
    withdraw_reserve_liquidity_fee_receiver: &Pubkey,
    obligation: &Pubkey,
    lending_market: &Pubkey,
    lending_market_authority: &Pubkey,
    user_transfer_authority: &Pubkey,
) -> Result<Instruction> {
    let program_id = get_program_id(env)?;
    
    // Instruction discriminator for LiquidateObligationAndRedeemReserveCollateral (instruction index 12)
    let mut data = vec![12];
    
    // Append liquidity amount as little-endian u64
    data.extend_from_slice(&liquidity_amount.to_le_bytes());
    
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(*repay_reserve_liquidity, false),
            AccountMeta::new(*withdraw_reserve_collateral_supply, false),
            AccountMeta::new(*withdraw_reserve_liquidity, false),
            AccountMeta::new(*repay_account, false),
            AccountMeta::new(*withdraw_collateral_account, false),
            AccountMeta::new(*withdraw_liquidity_account, false),
            AccountMeta::new(*repay_reserve, false),
            AccountMeta::new(*withdraw_reserve, false),
            AccountMeta::new(*obligation, false),
            AccountMeta::new_readonly(*lending_market, false),
            AccountMeta::new_readonly(*lending_market_authority, false),
            AccountMeta::new_readonly(*user_transfer_authority, true),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new(*withdraw_reserve_collateral_mint, false),
            AccountMeta::new(*withdraw_reserve_liquidity_fee_receiver, false),
        ],
        data,
    })
}
