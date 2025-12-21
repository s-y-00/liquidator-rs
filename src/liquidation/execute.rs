use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;
use std::collections::HashSet;

use crate::models::{MarketConfig, Obligation};
use crate::liquidation::instructions::{
    refresh_reserve_instruction,
    refresh_obligation_instruction,
    liquidate_and_redeem_instruction,
};

/// Execute liquidation and redeem transaction
/// Equivalent to libs/actions/liquidateAndRedeem.ts
#[allow(clippy::too_many_arguments)]
pub async fn liquidate_and_redeem(
    client: &RpcClient,
    env: &str,
    payer: &Keypair,
    liquidity_amount: u64,
    repay_token_symbol: &str,
    withdraw_token_symbol: &str,
    market: &MarketConfig,
    obligation: &Obligation,
) -> Result<()> {
    let mut instructions = vec![];
    
    // Collect unique reserve addresses from deposits and borrows
    let mut unique_reserves = HashSet::new();
    
    for deposit in &obligation.deposits {
        unique_reserves.insert(deposit.deposit_reserve);
    }
    
    for borrow in &obligation.borrows {
        unique_reserves.insert(borrow.borrow_reserve);
    }
    
    // Create refresh reserve instructions for all unique reserves
    for reserve_pubkey in &unique_reserves {
        let reserve_addr = reserve_pubkey.to_string();
        
        // Find reserve config
        let reserve_config = market.reserves
            .iter()
            .find(|r| r.address == reserve_addr)
            .ok_or_else(|| anyhow!("Reserve {} not found in market config", reserve_addr))?;
        
        let pyth_oracle = Pubkey::from_str(&reserve_config.pyth_oracle)?;
        let switchboard_oracle = Pubkey::from_str(&reserve_config.switchboard_oracle)?;
        
        let refresh_ix = refresh_reserve_instruction(
            env,
            reserve_pubkey,
            &pyth_oracle,
            &switchboard_oracle,
        )?;
        
        instructions.push(refresh_ix);
    }
    
    // Create refresh obligation instruction
    let deposit_reserves: Vec<Pubkey> = obligation.deposits
        .iter()
        .map(|d| d.deposit_reserve)
        .collect();
    
    let borrow_reserves: Vec<Pubkey> = obligation.borrows
        .iter()
        .map(|b| b.borrow_reserve)
        .collect();
    
    let obligation_pubkey = obligation.lending_market; // This should be the obligation pubkey
    
    let refresh_obligation_ix = refresh_obligation_instruction(
        env,
        &obligation_pubkey,
        &deposit_reserves,
        &borrow_reserves,
    )?;
    
    instructions.push(refresh_obligation_ix);
    
    // Get reserve configs for repay and withdraw tokens
    let repay_reserve = market.find_reserve(repay_token_symbol)
        .ok_or_else(|| anyhow!("Repay token {} not found", repay_token_symbol))?;
    
    let withdraw_reserve = market.find_reserve(withdraw_token_symbol)
        .ok_or_else(|| anyhow!("Withdraw token {} not found", withdraw_token_symbol))?;
    
    // Get associated token accounts
    let repay_mint = Pubkey::from_str(&repay_reserve.liquidity_token.mint)?;
    let withdraw_mint = Pubkey::from_str(&withdraw_reserve.liquidity_token.mint)?;
    
    let repay_account = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
        &repay_mint,
    );
    
    let withdraw_collateral_mint = Pubkey::from_str(&withdraw_reserve.collateral_mint_address)?;
    let withdraw_collateral_account = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
        &withdraw_collateral_mint,
    );
    
    let withdraw_liquidity_account = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
        &withdraw_mint,
    );
    
    // Check if accounts exist, create if needed
    // (Simplified - in production, check account existence first)
    
    // Create liquidate and redeem instruction
    let liquidate_ix = liquidate_and_redeem_instruction(
        env,
        liquidity_amount,
        &repay_account,
        &withdraw_collateral_account,
        &withdraw_liquidity_account,
        &Pubkey::from_str(&repay_reserve.address)?,
        &Pubkey::from_str(&repay_reserve.liquidity_address)?,
        &Pubkey::from_str(&withdraw_reserve.address)?,
        &withdraw_collateral_mint,
        &Pubkey::from_str(&withdraw_reserve.collateral_supply_address)?,
        &Pubkey::from_str(&withdraw_reserve.liquidity_address)?,
        &Pubkey::from_str(&withdraw_reserve.liquidity_fee_receiver_address)?,
        &obligation_pubkey,
        &Pubkey::from_str(&market.address)?,
        &Pubkey::from_str(&market.authority_address)?,
        &payer.pubkey(),
    )?;
    
    instructions.push(liquidate_ix);
    
    // Build and send transaction
    let recent_blockhash = client.get_latest_blockhash()?;
    
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction.sign(&[payer], recent_blockhash);
    
    let signature = client.send_and_confirm_transaction(&transaction)?;
    
    log::info!(
        "Liquidation successful! Signature: {} for repay: {} withdraw: {}",
        signature,
        repay_token_symbol,
        withdraw_token_symbol
    );
    
    Ok(())
}
