use anyhow::{anyhow, Result};
use log::{warn, info};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Minimum reasonable price (to catch zero/negative prices)
const MIN_PRICE: f64 = 0.000001;

/// Maximum reasonable price (to catch overflow/corrupted data)
const MAX_PRICE: f64 = 1_000_000_000.0;

/// Maximum slot age before price is considered stale (approximately 2 minutes at 400ms/slot)
const MAX_SLOT_AGE: u64 = 300;

/// Oracle price validation result
#[derive(Debug, Clone)]
pub struct PriceValidation {
    pub is_valid: bool,
    pub warnings: Vec<String>,
}

/// Validate a single oracle price
pub fn validate_price(
    symbol: &str,
    price: Decimal,
    slot: u64,
    current_slot: u64,
) -> PriceValidation {
    let mut warnings = Vec::new();
    let mut is_valid = true;
    
    let price_f64 = price.to_string().parse::<f64>().unwrap_or(0.0);
    
    // Check for zero or negative price
    if price_f64 <= 0.0 {
        warnings.push(format!("{}: Price is zero or negative ({})", symbol, price));
        is_valid = false;
    }
    
    // Check for NaN or infinite
    if price_f64.is_nan() || price_f64.is_infinite() {
        warnings.push(format!("{}: Price is NaN or infinite", symbol));
        is_valid = false;
    }
    
    // Check if price is within reasonable bounds
    if price_f64 < MIN_PRICE {
        warnings.push(format!(
            "{}: Price suspiciously low ({}, min: {})",
            symbol, price, MIN_PRICE
        ));
        warn!("⚠️  {}", warnings.last().unwrap());
    }
    
    if price_f64 > MAX_PRICE {
        warnings.push(format!(
            "{}: Price suspiciously high ({}, max: {})",
            symbol, price, MAX_PRICE
        ));
        warn!("⚠️  {}", warnings.last().unwrap());
    }
    
    // Check for stale price
    let slot_age = current_slot.saturating_sub(slot);
    if slot_age > MAX_SLOT_AGE {
        warnings.push(format!(
            "{}: Price may be stale (slot age: {}, max: {})",
            symbol, slot_age, MAX_SLOT_AGE
        ));
        warn!("⚠️  {}", warnings.last().unwrap());
    }
    
    PriceValidation { is_valid, warnings }
}

/// Validate all oracle prices and log warnings
pub fn validate_oracle_prices(
    prices: &HashMap<String, (Decimal, u64)>,
    current_slot: u64,
) -> Result<()> {
    let mut total_warnings = 0;
    let mut invalid_count = 0;
    
    for (symbol, (price, slot)) in prices {
        let validation = validate_price(symbol, *price, *slot, current_slot);
        
        if !validation.is_valid {
            invalid_count += 1;
        }
        
        total_warnings += validation.warnings.len();
    }
    
    if invalid_count > 0 {
        warn!(
            "⚠️  {} oracle prices failed validation (total warnings: {})",
            invalid_count, total_warnings
        );
    } else if total_warnings > 0 {
        info!(
            "✓ All oracle prices valid, but {} warnings issued",
            total_warnings
        );
    } else {
        info!("✓ All oracle prices validated successfully");
    }
    
    Ok(())
}

/// Compare prices from different oracle sources
pub fn compare_oracle_sources(
    symbol: &str,
    pyth_price: Option<Decimal>,
    switchboard_price: Option<Decimal>,
    tolerance_percent: f64,
) -> Result<()> {
    if let (Some(pyth), Some(sb)) = (pyth_price, switchboard_price) {
        let pyth_f64 = pyth.to_string().parse::<f64>()?;
        let sb_f64 = sb.to_string().parse::<f64>()?;
        
        let diff_percent = ((pyth_f64 - sb_f64).abs() / pyth_f64) * 100.0;
        
        if diff_percent > tolerance_percent {
            warn!(
                "⚠️  {}: Large price discrepancy between oracles (Pyth: {}, Switchboard: {}, diff: {:.2}%)",
                symbol, pyth, sb, diff_percent
            );
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    
    #[test]
    fn test_validate_price_valid() {
        let validation = validate_price("SOL", dec!(100.50), 1000, 1100);
        assert!(validation.is_valid);
        assert!(validation.warnings.is_empty());
    }
    
    #[test]
    fn test_validate_price_zero() {
        let validation = validate_price("SOL", dec!(0.0), 1000, 1100);
        assert!(!validation.is_valid);
        assert!(!validation.warnings.is_empty());
    }
    
    #[test]
    fn test_validate_price_negative() {
        let validation = validate_price("SOL", dec!(-10.0), 1000, 1100);
        assert!(!validation.is_valid);
        assert!(!validation.warnings.is_empty());
    }
    
    #[test]
    fn test_price_staleness() {
        // Price is 400 slots old (stale)
        let validation = validate_price("SOL", dec!(100.0), 1000, 1400);
        assert!(!validation.warnings.is_empty());
    }
    
    #[test]
    fn test_price_too_low() {
        let validation = validate_price("SOL", dec!(0.0000001), 1000, 1100);
        assert!(!validation.warnings.is_empty());
    }
    
    #[test]
    fn test_price_too_high() {
        let validation = validate_price("SOL", dec!(2000000000.0), 1000, 1100);
        assert!(!validation.warnings.is_empty());
    }
}
