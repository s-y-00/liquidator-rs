use serde::{Deserialize, Serialize};

/// Market configuration from Solend API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketConfig {
    pub name: String,
    pub is_primary: bool,
    pub description: String,
    pub creator: String,
    pub address: String,
    pub authority_address: String,
    pub owner: String,
    pub reserves: Vec<MarketConfigReserve>,
}

/// Reserve configuration within a market
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketConfigReserve {
    pub liquidity_token: LiquidityToken,
    pub pyth_oracle: String,
    pub switchboard_oracle: String,
    pub address: String,
    pub collateral_mint_address: String,
    pub collateral_supply_address: String,
    pub liquidity_address: String,
    pub liquidity_fee_receiver_address: String,
    pub user_supply_cap: u64,
}

/// Liquidity token metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidityToken {
    pub coingecko_id: String,
    pub decimals: u8,
    pub logo: String,
    pub mint: String,
    pub name: String,
    pub symbol: String,
    pub volume24h: String,
}

/// Target token distribution for wallet rebalancing
#[derive(Debug, Clone)]
pub struct TokenCount {
    pub symbol: String,
    pub target: f64,
}

impl MarketConfig {
    /// Find reserve by token symbol
    pub fn find_reserve(&self, symbol: &str) -> Option<&MarketConfigReserve> {
        self.reserves
            .iter()
            .find(|r| r.liquidity_token.symbol == symbol)
    }
    
    /// Get all token symbols in this market
    pub fn token_symbols(&self) -> Vec<String> {
        self.reserves
            .iter()
            .map(|r| r.liquidity_token.symbol.clone())
            .collect()
    }
}

impl MarketConfigReserve {
    /// Get token decimals
    pub fn decimals(&self) -> u8 {
        self.liquidity_token.decimals
    }
    
    /// Get token mint address
    pub fn mint_address(&self) -> &str {
        &self.liquidity_token.mint
    }
}
