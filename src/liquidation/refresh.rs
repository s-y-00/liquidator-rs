use anyhow::Result;
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::models::{Obligation, Reserve};
use crate::oracle::TokenOracleData;

/// Refreshed obligation data with calculated values
#[derive(Debug, Clone)]
pub struct RefreshedObligation {
    pub borrowed_value: Decimal,
    pub unhealthy_borrow_value: Decimal,
    pub deposits: Vec<RefreshedDeposit>,
    pub borrows: Vec<RefreshedBorrow>,
}

#[derive(Debug, Clone)]
pub struct RefreshedDeposit {
    pub deposit_reserve: String,
    pub deposited_amount: u64,
    pub market_value: Decimal,
    pub symbol: String,
    pub mint_address: String,
}

#[derive(Debug, Clone)]
pub struct RefreshedBorrow {
    pub borrow_reserve: String,
    pub borrowed_amount_wads: u128,
    pub market_value: Decimal,
    pub symbol: String,
    pub mint_address: String,
}

/// Calculate refreshed obligation health
/// Equivalent to libs/refreshObligation.ts:calculateRefreshedObligation
pub fn calculate_refreshed_obligation(
    obligation: &Obligation,
    reserves: &[(String, Reserve)],
    oracle_data: &HashMap<String, TokenOracleData>,
) -> Result<RefreshedObligation> {
    // Create reserve lookup map
    let reserve_map: HashMap<String, &Reserve> = reserves
        .iter()
        .map(|(addr, reserve)| (addr.clone(), reserve))
        .collect();
    
    let mut total_borrowed_value = Decimal::ZERO;
    let mut total_allowed_borrow_value = Decimal::ZERO;
    let mut total_unhealthy_borrow_value = Decimal::ZERO;
    
    let mut refreshed_deposits = Vec::new();
    let mut refreshed_borrows = Vec::new();
    
    // Process deposits
    for deposit in &obligation.deposits {
        let reserve_addr = deposit.deposit_reserve.to_string();
        
        if let Some(reserve) = reserve_map.get(&reserve_addr) {
            // Find oracle data by matching reserve liquidity mint
            let mint_addr = reserve.liquidity.mint_pubkey.to_string();
            
            if let Some(oracle) = oracle_data.values().find(|o| o.mint_address == mint_addr) {
                let deposited_amount = deposit.deposited_amount;
                
                // Calculate collateral exchange rate
                let exchange_rate = reserve.get_collateral_exchange_rate();
                
                // Calculate liquidity amount from collateral
                let liquidity_amount = Decimal::from(deposited_amount) / exchange_rate;
                
                // Calculate market value
                let market_value = liquidity_amount * oracle.price / Decimal::from(oracle.decimals);
                
                // Add to allowed borrow value
                let ltv = reserve.get_loan_to_value_rate();
                total_allowed_borrow_value += market_value * ltv;
                
                // Add to unhealthy borrow value
                let liquidation_threshold = reserve.get_liquidation_threshold_rate();
                total_unhealthy_borrow_value += market_value * liquidation_threshold;
                
                refreshed_deposits.push(RefreshedDeposit {
                    deposit_reserve: reserve_addr.clone(),
                    deposited_amount,
                    market_value,
                    symbol: oracle.symbol.clone(),
                    mint_address: mint_addr,
                });
            }
        }
    }
    
    // Process borrows
    for borrow in &obligation.borrows {
        let reserve_addr = borrow.borrow_reserve.to_string();
        
        if let Some(reserve) = reserve_map.get(&reserve_addr) {
            let mint_addr = reserve.liquidity.mint_pubkey.to_string();
            
            if let Some(oracle) = oracle_data.values().find(|o| o.mint_address == mint_addr) {
                let borrowed_amount_wads = borrow.borrowed_amount_wads;
                
                // Calculate actual borrowed amount from wads
                let wad = Decimal::from(crate::utils::WAD);
                let borrowed_amount = Decimal::from(borrowed_amount_wads) / wad;
                
                // Calculate market value
                let market_value = borrowed_amount * oracle.price / Decimal::from(oracle.decimals);
                
                total_borrowed_value += market_value;
                
                refreshed_borrows.push(RefreshedBorrow {
                    borrow_reserve: reserve_addr.clone(),
                    borrowed_amount_wads,
                    market_value,
                    symbol: oracle.symbol.clone(),
                    mint_address: mint_addr,
                });
            }
        }
    }
    
    Ok(RefreshedObligation {
        borrowed_value: total_borrowed_value,
        unhealthy_borrow_value: total_unhealthy_borrow_value,
        deposits: refreshed_deposits,
        borrows: refreshed_borrows,
    })
}

impl RefreshedObligation {
    /// Check if obligation is underwater (unhealthy)
    pub fn is_unhealthy(&self) -> bool {
        self.borrowed_value > self.unhealthy_borrow_value
    }
    
    /// Get the best borrow to repay (highest market value)
    pub fn select_repay_borrow(&self) -> Option<&RefreshedBorrow> {
        self.borrows
            .iter()
            .max_by(|a, b| a.market_value.cmp(&b.market_value))
    }
    
    /// Get the best collateral to withdraw (highest market value)
    pub fn select_withdraw_deposit(&self) -> Option<&RefreshedDeposit> {
        self.deposits
            .iter()
            .max_by(|a, b| a.market_value.cmp(&b.market_value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refreshed_obligation_healthy() {
        let refreshed = RefreshedObligation {
            borrowed_value: Decimal::from(100),
            unhealthy_borrow_value: Decimal::from(120),
            deposits: vec![],
            borrows: vec![],
        };
        
        assert!(!refreshed.is_unhealthy());
    }

    #[test]
    fn test_refreshed_obligation_unhealthy() {
        let refreshed = RefreshedObligation {
            borrowed_value: Decimal::from(150),
            unhealthy_borrow_value: Decimal::from(120),
            deposits: vec![],
            borrows: vec![],
        };
        
        assert!(refreshed.is_unhealthy());
    }
}
