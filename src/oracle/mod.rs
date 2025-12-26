pub mod pyth;
pub mod switchboard;
pub mod validation;

use anyhow::Result;
use std::collections::HashMap;

use crate::models::MarketConfig;
use crate::rpc::SolendRpcClient;

pub use pyth::TokenOracleData;
pub use pyth::NULL_ORACLE;

use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Fetch oracle data for all tokens in a market
/// Optimized to use batch fetching (1 RPC call instead of N)
pub async fn get_tokens_oracle_data(
    client: &SolendRpcClient,
    market: &MarketConfig,
) -> Result<HashMap<String, TokenOracleData>> {
    let mut oracle_data = HashMap::new();
    let mut oracle_requests = Vec::new();
    
    // 1. Collect all oracle addresses to fetch
    for reserve in &market.reserves {
        let oracle_addr_str = if reserve.pyth_oracle != pyth::NULL_ORACLE {
            &reserve.pyth_oracle
        } else if reserve.switchboard_oracle != pyth::NULL_ORACLE {
            // For now, we only batch fetch Pyth. Switchboard is handled individually if needed,
            // or we could add it here if we implemented parsing.
            // Given the placeholder status of Switchboard, we'll focus on Pyth.
            &reserve.switchboard_oracle
        } else {
            log::warn!("No valid oracle for {}", reserve.liquidity_token.symbol);
            continue;
        };
        
        if let Ok(pubkey) = Pubkey::from_str(oracle_addr_str) {
            oracle_requests.push((reserve, pubkey, oracle_addr_str.clone()));
        }
    }
    
    if oracle_requests.is_empty() {
        return Ok(oracle_data);
    }
    
    // 2. Fetch all accounts in batches
    let pubkeys: Vec<Pubkey> = oracle_requests.iter().map(|(_, pk, _)| *pk).collect();
    let accounts = client.get_multiple_accounts_batched(&pubkeys, 100).await?;
    
    // 3. Parse results
    for ((reserve, _, _), account_opt) in oracle_requests.iter().zip(accounts.iter()) {
        if let Some(account) = account_opt {
            // Currently assuming Pyth for parsing as it's the primary oracle
            // If Switchboard becomes active, we'd need to distinguish based on the address source
            if reserve.pyth_oracle != pyth::NULL_ORACLE {
                match pyth::parse_price_from_account(account) {
                    Ok(price) => {
                        let data = TokenOracleData {
                            symbol: reserve.liquidity_token.symbol.clone(),
                            reserve_address: reserve.address.clone(),
                            mint_address: reserve.liquidity_token.mint.clone(),
                            decimals: 10u32.pow(reserve.liquidity_token.decimals as u32),
                            price,
                        };
                        oracle_data.insert(data.symbol.clone(), data);
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to parse Pyth oracle for {}: {}",
                            reserve.liquidity_token.symbol,
                            e
                        );
                    }
                }
            } else {
                 // Fallback for Switchboard (if we fetched it) - logic similar to before
                 // Since we don't have parsing logic exposed properly for Switchboard yet,
                 // and it's a placeholder, we skip or could implement a basic check.
            }
        } else {
            log::warn!("Oracle account not found for {}", reserve.liquidity_token.symbol);
        }
    }
    
    log::info!("Fetched oracle data for {} tokens (batched)", oracle_data.len());
    Ok(oracle_data)
}
