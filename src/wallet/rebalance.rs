use anyhow::{anyhow, Result};
use log::{info, warn};
use rust_decimal::prelude::ToPrimitive;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::{Keypair, Signer}};
use std::collections::HashMap;

use crate::models::market::TokenCount;
use crate::wallet::balance::get_wallet_token_balance;
use crate::wallet::swap::{get_usdc_mint, JupiterClient};

/// Calculate which tokens need rebalancing
pub fn calculate_rebalance_needed(
    current_balances: &HashMap<String, f64>,
    targets: &[TokenCount],
    padding: f64,
) -> Vec<(String, f64, bool)> {
    let mut rebalance_actions = Vec::new();
    
    for target in targets {
        let current = current_balances.get(&target.symbol).copied().unwrap_or(0.0);
        let target_amount = target.target;
        
        // Calculate bounds with padding
        let lower_bound = target_amount * (1.0 - padding);
        let upper_bound = target_amount * (1.0 + padding);
        
        // Skip USDC as it's the base token
        if target.symbol == "USDC" {
            continue;
        }
        
        // Check if rebalancing is needed
        if current < lower_bound {
            // Need to buy (swap USDC -> token)
            let amount_needed = target_amount - current;
            rebalance_actions.push((target.symbol.clone(), amount_needed, true));
        } else if current > upper_bound {
            // Need to sell (swap token -> USDC)
            let amount_to_sell = current - target_amount;
            rebalance_actions.push((target.symbol.clone(), amount_to_sell, false));
        }
    }
    
    rebalance_actions
}

/// Rebalance wallet to maintain target distribution
pub async fn rebalance_wallet(
    rpc_client: &RpcClient,
    payer: &Keypair,
    env: &str,
    targets: &[TokenCount],
    padding: f64,
    token_mints: &HashMap<String, (Pubkey, u8)>, // symbol -> (mint, decimals)
) -> Result<()> {
    if targets.is_empty() {
        return Ok(());
    }
    
    info!("Starting wallet rebalancing...");
    
    // Get current balances
    let mut current_balances = HashMap::new();
    for (symbol, (mint, decimals)) in token_mints {
        let (_, balance_decimal) = get_wallet_token_balance(
            rpc_client,
            mint,
            &payer.pubkey(),
            *decimals,
        )?;
        let balance_f64 = balance_decimal.to_f64().unwrap_or(0.0);
        current_balances.insert(symbol.clone(), balance_f64);
    }
    
    // Calculate rebalancing actions
    let actions = calculate_rebalance_needed(&current_balances, targets, padding);
    
    if actions.is_empty() {
        info!("✓ Wallet is balanced, no rebalancing needed");
        return Ok(());
    }
    
    info!("Rebalancing {} tokens", actions.len());
    
    let jupiter = JupiterClient::new();
    let usdc_mint = get_usdc_mint(env)?;
    
    // Execute rebalancing swaps
    for (symbol, amount, is_buy) in actions {
        let (token_mint, decimals) = token_mints
            .get(&symbol)
            .ok_or_else(|| anyhow!("Token {} not found in mint map", symbol))?;
        
        if is_buy {
            info!("  Buying {:.4} {} (swapping USDC)", amount, symbol);
            
            // Calculate USDC amount needed (approximate)
            // In production, you'd get a quote first to determine exact amount
            let usdc_decimals = 6;
            let usdc_amount = (amount * 100.0 * 10f64.powi(usdc_decimals as i32)) as u64; // Rough estimate
            
            match jupiter.swap(
                rpc_client,
                payer,
                &usdc_mint,
                token_mint,
                usdc_amount,
                100, // 1% slippage
            ).await {
                Ok(sig) => info!("    ✓ Bought {} (sig: {})", symbol, sig),
                Err(e) => warn!("    ✗ Failed to buy {}: {}", symbol, e),
            }
        } else {
            info!("  Selling {:.4} {} (swapping to USDC)", amount, symbol);
            
            let token_amount = (amount * 10f64.powi(*decimals as i32)) as u64;
            
            match jupiter.swap(
                rpc_client,
                payer,
                token_mint,
                &usdc_mint,
                token_amount,
                100, // 1% slippage
            ).await {
                Ok(sig) => info!("    ✓ Sold {} (sig: {})", symbol, sig),
                Err(e) => warn!("    ✗ Failed to sell {}: {}", symbol, e),
            }
        }
    }
    
    info!("✓ Wallet rebalancing complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_rebalance_needed() {
        let mut current = HashMap::new();
        current.insert("SOL".to_string(), 0.3);
        current.insert("USDT".to_string(), 60.0);
        
        let targets = vec![
            TokenCount { symbol: "USDC".to_string(), target: 100.0 },
            TokenCount { symbol: "SOL".to_string(), target: 1.0 },
            TokenCount { symbol: "USDT".to_string(), target: 50.0 },
        ];
        
        let actions = calculate_rebalance_needed(&current, &targets, 0.2);
        
        // SOL: current 0.3, target 1.0, lower bound 0.8 -> need to buy
        // USDT: current 60.0, target 50.0, upper bound 60.0 -> at boundary, no action
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].0, "SOL");
        assert!(actions[0].2); // is_buy
    }
    
    #[test]
    fn test_no_rebalance_within_threshold() {
        let mut current = HashMap::new();
        current.insert("SOL".to_string(), 0.9);
        
        let targets = vec![
            TokenCount { symbol: "SOL".to_string(), target: 1.0 },
        ];
        
        let actions = calculate_rebalance_needed(&current, &targets, 0.2);
        
        // 0.9 is within [0.8, 1.2], no rebalancing needed
        assert_eq!(actions.len(), 0);
    }
}
