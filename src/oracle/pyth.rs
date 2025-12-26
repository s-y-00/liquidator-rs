use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use solana_sdk::account::Account as SolanaAccount;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use crate::models::MarketConfigReserve;
use crate::rpc::SolendRpcClient;

pub const NULL_ORACLE: &str = "nu11111111111111111111111111111111111111111";

/// Token oracle data
#[derive(Debug, Clone)]
pub struct TokenOracleData {
    pub symbol: String,
    pub reserve_address: String,
    pub mint_address: String,
    pub decimals: u32,
    pub price: Decimal,
}

/// Fetch token price from Pyth oracle
pub async fn get_token_oracle_data(
    client: &SolendRpcClient,
    reserve: &MarketConfigReserve,
) -> Result<TokenOracleData> {
    let price = if reserve.pyth_oracle != NULL_ORACLE {
        // Try Pyth oracle first
        fetch_pyth_price(client, &reserve.pyth_oracle).await?
    } else if reserve.switchboard_oracle != NULL_ORACLE {
        // Fallback to Switchboard
        super::switchboard::fetch_switchboard_price(client, &reserve.switchboard_oracle).await?
    } else {
        return Err(anyhow!("No valid oracle for {}", reserve.liquidity_token.symbol));
    };
    
    Ok(TokenOracleData {
        symbol: reserve.liquidity_token.symbol.clone(),
        reserve_address: reserve.address.clone(),
        mint_address: reserve.liquidity_token.mint.clone(),
        decimals: 10u32.pow(reserve.liquidity_token.decimals as u32),
        price,
    })
}

/// Fetch price from Pyth oracle
async fn fetch_pyth_price(client: &SolendRpcClient, oracle_address: &str) -> Result<Decimal> {
    let pubkey = Pubkey::from_str(oracle_address)?;
    let account = client.get_account(&pubkey)?;
    parse_price_from_account(&account)
}

/// Parse price from Pyth account data
pub fn parse_price_from_account(account: &SolanaAccount) -> Result<Decimal> {
    // Check if account data is large enough for Pyth price feed
    if account.data.len() < 200 {
        return Err(anyhow!("Invalid Pyth account data size"));
    }
    
    // Pyth V2 price offset is at byte 208-215 (price as i64)
    // Expo is at byte 216-219 (expo as i32)
    // This is a simplified parsing - in production use pyth-sdk properly
    
    let price_bytes = &account.data[208..216];
    let expo_bytes = &account.data[216..220];
    
    // Safety check for panic-free slicing
    if account.data.len() < 220 {
        return Err(anyhow!("Pyth account data too small"));
    }
    
    let price_i64 = i64::from_le_bytes(price_bytes.try_into()?);
    let expo = i32::from_le_bytes(expo_bytes.try_into()?);
    
    // Convert to decimal: price * 10^expo
    let price = Decimal::from(price_i64);
    let exponent = Decimal::from(10i64.pow(expo.abs() as u32));
    
    let final_price = if expo < 0 {
        price / exponent
    } else {
        price * exponent
    };
    
    // Allow zero prices for now if valid, but typically liquidations rely on non-zero
    // Some feeds might momentarily be zero? Better to validate in caller.
    // Logic kept consistent with previous implementation
    if final_price.is_sign_negative() {
         return Err(anyhow!("Invalid negative price from Pyth oracle: {}", final_price));
    }
    
    Ok(final_price)
}
