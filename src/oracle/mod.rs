pub mod pyth;
pub mod switchboard;

use anyhow::Result;
use std::collections::HashMap;

use crate::models::MarketConfig;
use crate::rpc::SolendRpcClient;

pub use pyth::TokenOracleData;

/// Fetch oracle data for all tokens in a market
pub async fn get_tokens_oracle_data(
    client: &SolendRpcClient,
    market: &MarketConfig,
) -> Result<HashMap<String, TokenOracleData>> {
    let mut oracle_data = HashMap::new();
    
    for reserve in &market.reserves {
        match pyth::get_token_oracle_data(client, reserve).await {
            Ok(data) => {
                oracle_data.insert(data.symbol.clone(), data);
            }
            Err(e) => {
                log::error!(
                    "Failed to fetch oracle data for {}: {}",
                    reserve.liquidity_token.symbol,
                    e
                );
            }
        }
    }
    
    log::info!("Fetched oracle data for {} tokens", oracle_data.len());
    Ok(oracle_data)
}
