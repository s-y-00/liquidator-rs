use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::time::Duration;

/// WAD constant - 10^18 for high precision calculations
pub const WAD: u128 = 1_000_000_000_000_000_000;

/// U64 max value as string
pub const U64_MAX: &str = "18446744073709551615";

/// Program IDs for different deployments
pub const PROGRAM_ID_PRODUCTION: &str = "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo";
pub const PROGRAM_ID_BETA: &str = "BLendhFh4HGnycEDDFhbeFEUYLP4fXB5tTHMoTX8Dch5";
pub const PROGRAM_ID_STAGING: &str = "ALend7Ketfx5bxh6ghsCDXAoDrhvEmsXT3cynB6aPLgx";

/// Get program ID based on environment
pub fn get_program_id(env: &str) -> Result<Pubkey> {
    let program_id_str = match env {
        "production" => PROGRAM_ID_PRODUCTION,
        "beta" => PROGRAM_ID_BETA,
        "staging" => PROGRAM_ID_STAGING,
        _ => PROGRAM_ID_PRODUCTION,
    };
    
    Pubkey::from_str(program_id_str)
        .map_err(|e| anyhow!("Failed to parse program ID: {}", e))
}

/// Convert base unit amount to human-readable format with decimals
pub fn to_human(amount: u64, decimals: u8) -> Decimal {
    let amount_decimal = Decimal::from(amount);
    let divisor = Decimal::from(10u64.pow(decimals as u32));
    amount_decimal / divisor
}

/// Convert human-readable amount to base units
pub fn to_base_unit(amount: &str, decimals: u8) -> Result<u64> {
    if amount == U64_MAX {
        return Ok(u64::MAX);
    }
    
    let amount_decimal = Decimal::from_str(amount)?;
    let multiplier = Decimal::from(10u64.pow(decimals as u32));
    let result = amount_decimal * multiplier;
    
    result
        .to_u64()
        .ok_or_else(|| anyhow!("Amount overflow when converting to base units"))
}

/// Async sleep utility
pub async fn wait(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// Strip trailing character from string
pub fn strip_end(s: &str, c: char) -> String {
    s.trim_end_matches(c).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_human() {
        // 1 SOL = 1_000_000_000 lamports (9 decimals)
        let result = to_human(1_000_000_000, 9);
        assert_eq!(result, Decimal::from(1));
        
        // 0.5 SOL
        let result = to_human(500_000_000, 9);
        assert_eq!(result, Decimal::new(5, 1));
    }

    #[test]
    fn test_to_base_unit() {
        // 1 SOL to lamports
        let result = to_base_unit("1", 9).unwrap();
        assert_eq!(result, 1_000_000_000);
        
        // 0.5 SOL to lamports
        let result = to_base_unit("0.5", 9).unwrap();
        assert_eq!(result, 500_000_000);
    }

    #[test]
    fn test_get_program_id() {
        let prod_id = get_program_id("production").unwrap();
        assert_eq!(prod_id.to_string(), PROGRAM_ID_PRODUCTION);
    }
}
