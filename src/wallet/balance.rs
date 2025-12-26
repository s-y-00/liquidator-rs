use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use rust_decimal::Decimal;

use crate::utils::to_human;

/// Get associated token address for a mint and wallet
pub fn find_associated_token_address(
    wallet_address: &Pubkey,
    token_mint_address: &Pubkey,
) -> Pubkey {
    spl_associated_token_account::get_associated_token_address(
        wallet_address,
        token_mint_address,
    )
}

/// Get wallet token balance
pub fn get_wallet_token_balance(
    client: &RpcClient,
    mint: &Pubkey,
    wallet_address: &Pubkey,
    decimals: u8,
) -> Result<(u64, Decimal)> {
    let ata = find_associated_token_address(wallet_address, mint);
    
    match client.get_token_account_balance(&ata) {
        Ok(token_amount) => {
            let balance_base = token_amount
                .amount
                .parse::<u64>()
                .map_err(|e| anyhow!("Failed to parse token amount: {}", e))?;
            
            let balance_human = to_human(balance_base, decimals);
            
            Ok((balance_base, balance_human))
        }
        Err(_) => {
            // Account doesn't exist or error fetching
            Ok((0, Decimal::ZERO))
        }
    }
}

/// Get multiple wallet token balances in batches
/// Returns a map of Mint Pubkey -> (Balance Base, Balance Human)
pub async fn get_wallet_token_balances_batched(
    client: &crate::rpc::SolendRpcClient,
    wallet_address: &Pubkey,
    mints: &[Pubkey],
    decimals_map: &std::collections::HashMap<Pubkey, u8>,
) -> Result<std::collections::HashMap<Pubkey, (u64, Decimal)>> {
    use solana_sdk::program_pack::Pack;
    use spl_token::state::Account as TokenAccount;
    
    // 1. Derive ATAs for all mints
    let atas: Vec<Pubkey> = mints
        .iter()
        .map(|mint| find_associated_token_address(wallet_address, mint))
        .collect();
        
    // 2. Batch fetch all ATAs
    // Use the batched method we added to RpcClient
    let accounts = client.get_multiple_accounts_batched(&atas, 100).await?;
    
    let mut results = std::collections::HashMap::new();
    
    // 3. Parse accounts
    for (i, account_opt) in accounts.iter().enumerate() {
        let mint = &mints[i];
        let decimals = *decimals_map.get(mint).unwrap_or(&9); // Default to 9 if not found
        
        let (balance_base, balance_human) = if let Some(account) = account_opt {
            if let Ok(token_account) = TokenAccount::unpack(&account.data) {
                let amount = token_account.amount;
                (amount, to_human(amount, decimals))
            } else {
                // Failed to unpack
                (0, Decimal::ZERO)
            }
        } else {
            // Account doesn't exist (balance 0)
            (0, Decimal::ZERO)
        };
        
        results.insert(*mint, (balance_base, balance_human));
    }
    
    Ok(results)
}

/// Wallet balance data
#[derive(Debug, Clone)]
pub struct WalletTokenData {
    pub symbol: String,
    pub balance_base: u64,
    pub balance: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_find_associated_token_address() {
        let wallet = Pubkey::from_str("11111111111111111111111111111111").unwrap();
        let mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(); // USDC
        
        let ata = find_associated_token_address(&wallet, &mint);
        
        // ATA should be derived deterministically
        assert_ne!(ata, Pubkey::default());
    }
}
