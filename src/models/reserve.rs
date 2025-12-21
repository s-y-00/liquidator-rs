use borsh::{BorshDeserialize, BorshSerialize};
use rust_decimal::Decimal;
use solana_sdk::pubkey::Pubkey;
use super::last_update::LastUpdate;

/// Reserve account size
pub const RESERVE_SIZE: usize = 619;

/// WAD constant for reserve calculations
pub const WAD: u128 = 1_000_000_000_000_000_000;

/// Reserve account data
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct Reserve {
    pub version: u8,
    pub last_update: LastUpdate,
    pub lending_market: Pubkey,
    pub liquidity: ReserveLiquidity,
    pub collateral: ReserveCollateral,
    pub config: ReserveConfig,
}

/// Reserve liquidity state
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ReserveLiquidity {
    pub mint_pubkey: Pubkey,
    pub mint_decimals: u8,
    pub supply_pubkey: Pubkey,
    pub pyth_oracle_pubkey: Pubkey,
    pub switchboard_oracle_pubkey: Pubkey,
    pub available_amount: u64,
    pub borrowed_amount_wads: u128,
    pub cumulative_borrow_rate_wads: u128,
    pub market_price: u128,
}

/// Reserve collateral state
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ReserveCollateral {
    pub mint_pubkey: Pubkey,
    pub mint_total_supply: u64,
    pub supply_pubkey: Pubkey,
}

/// Reserve fees configuration
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ReserveFees {
    pub borrow_fee_wad: u64,
    pub flash_loan_fee_wad: u64,
    pub host_fee_percentage: u8,
}

/// Reserve configuration parameters
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ReserveConfig {
    pub optimal_utilization_rate: u8,
    pub loan_to_value_ratio: u8,
    pub liquidation_bonus: u8,
    pub liquidation_threshold: u8,
    pub min_borrow_rate: u8,
    pub optimal_borrow_rate: u8,
    pub max_borrow_rate: u8,
    pub fees: ReserveFees,
    pub deposit_limit: u64,
    pub borrow_limit: u64,
    pub fee_receiver: Pubkey,
}

impl Reserve {
    /// Parse reserve from account data
    pub fn parse(data: &[u8]) -> Result<Self, std::io::Error> {
        if data.len() < RESERVE_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid reserve data size",
            ));
        }
        
        // Deserialize using Borsh
        // Note: In production, ensure exact layout matches Solend program
        Self::deserialize(&mut &data[..])
    }
    
    /// Calculate collateral exchange rate
    pub fn get_collateral_exchange_rate(&self) -> Decimal {
        let total_liquidity = Decimal::from(self.liquidity.available_amount) 
            * Decimal::from(WAD)
            + Decimal::from(self.liquidity.borrowed_amount_wads);
        
        if self.collateral.mint_total_supply == 0 || total_liquidity.is_zero() {
            // Initial collateral ratio
            Decimal::from(WAD)
        } else {
            let mint_supply = Decimal::from(self.collateral.mint_total_supply);
            (mint_supply * Decimal::from(WAD)) / total_liquidity
        }
    }
    
    /// Get loan-to-value ratio as decimal
    pub fn get_loan_to_value_rate(&self) -> Decimal {
        Decimal::from(self.config.loan_to_value_ratio) / Decimal::from(100)
    }
    
    /// Get liquidation threshold as decimal
    pub fn get_liquidation_threshold_rate(&self) -> Decimal {
        Decimal::from(self.config.liquidation_threshold) / Decimal::from(100)
    }
}
