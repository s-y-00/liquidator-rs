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
