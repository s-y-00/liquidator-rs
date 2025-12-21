use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;
use super::last_update::LastUpdate;

/// Obligation account size
pub const OBLIGATION_SIZE: usize = 1300;

/// Obligation account data
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct Obligation {
    pub version: u8,
    pub last_update: LastUpdate,
    pub lending_market: Pubkey,
    pub owner: Pubkey,
    pub deposited_value: u128,
    pub borrowed_value: u128,
    pub allowed_borrow_value: u128,
    pub unhealthy_borrow_value: u128,
    pub deposits: Vec<ObligationCollateral>,
    pub borrows: Vec<ObligationLiquidity>,
}

/// Collateral deposited in an obligation
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ObligationCollateral {
    pub deposit_reserve: Pubkey,
    pub deposited_amount: u64,
    pub market_value: u128,
}

/// Liquidity borrowed in an obligation
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ObligationLiquidity {
    pub borrow_reserve: Pubkey,
    pub cumulative_borrow_rate_wads: u128,
    pub borrowed_amount_wads: u128,
    pub market_value: u128,
}

impl Obligation {
    /// Parse obligation from account data
    /// Note: Solend uses custom layout, may need manual parsing
    pub fn parse(data: &[u8]) -> Result<Self, std::io::Error> {
        if data.len() < OBLIGATION_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid obligation data size",
            ));
        }
        
        // Custom parsing logic for Solend obligation layout
        // This is a simplified version - actual implementation needs to match exact layout
        let mut reader = &data[..];
        
        let version = u8::deserialize(&mut reader)?;
        let last_update = LastUpdate::deserialize(&mut reader)?;
        
        // Skip to avoid complex layout parsing for now
        // In production, use exact offset calculations matching the TypeScript version
        let lending_market = Pubkey::deserialize(&mut reader)?;
        let owner = Pubkey::deserialize(&mut reader)?;
        let deposited_value = u128::deserialize(&mut reader)?;
        let borrowed_value = u128::deserialize(&mut reader)?;
        let allowed_borrow_value = u128::deserialize(&mut reader)?;
        let unhealthy_borrow_value = u128::deserialize(&mut reader)?;
        
        // Skip padding (64 bytes)
        reader = &reader[64..];
        
        let deposits_len = u8::deserialize(&mut reader)?;
        let borrows_len = u8::deserialize(&mut reader)?;
        
        // Parse deposits and borrows from data_flat
        let mut deposits = Vec::new();
        let mut borrows = Vec::new();
        
        for _ in 0..deposits_len {
            deposits.push(ObligationCollateral::deserialize(&mut reader)?);
        }
        
        for _ in 0..borrows_len {
            borrows.push(ObligationLiquidity::deserialize(&mut reader)?);
        }
        
        Ok(Obligation {
            version,
            last_update,
            lending_market,
            owner,
            deposited_value,
            borrowed_value,
            allowed_borrow_value,
            unhealthy_borrow_value,
            deposits,
            borrows,
        })
    }
    
    /// Check if obligation is healthy
    pub fn is_healthy(&self) -> bool {
        self.borrowed_value <= self.unhealthy_borrow_value
    }
}
