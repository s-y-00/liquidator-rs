use anyhow::{anyhow, Result};
use reqwest;
use std::env;
use std::time::Duration;

use crate::models::market::{MarketConfig, TokenCount};

/// Configuration for the liquidator bot
#[derive(Debug, Clone)]
pub struct Config {
    pub app: String,
    pub rpc_endpoint: String,
    pub secret_path: String,
    pub markets_filter: Option<String>,
    pub targets: Vec<TokenCount>,
    pub throttle_ms: u64,
    pub rebalance_padding: f64,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        // Load .env file if it exists
        let _ = dotenv::dotenv();
        
        let app = env::var("APP").unwrap_or_else(|_| "production".to_string());
        
        if !["production", "devnet", "beta", "staging"].contains(&app.as_str()) {
            return Err(anyhow!(
                "Unrecognized env app provided: {}. Must be production, devnet, beta, or staging",
                app
            ));
        }
        
        let rpc_endpoint = env::var("RPC_ENDPOINT")
            .map_err(|_| anyhow!("RPC_ENDPOINT must be set in environment"))?;
        
        let secret_path = env::var("SECRET_PATH")
            .map_err(|_| anyhow!("SECRET_PATH must be set in environment"))?;
        
        let markets_filter = env::var("MARKETS").ok();
        
        let targets = Self::parse_targets(&env::var("TARGETS").unwrap_or_default());
        
        let throttle_ms = env::var("THROTTLE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        
        let rebalance_padding = env::var("REBALANCE_PADDING")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.2);
        
        Ok(Config {
            app,
            rpc_endpoint,
            secret_path,
            markets_filter,
            targets,
            throttle_ms,
            rebalance_padding,
        })
    }
    
    /// Parse target distribution from TARGETS env var
    /// Format: "USDC:100 USDT:5 SOL:0.5"
    fn parse_targets(targets_str: &str) -> Vec<TokenCount> {
        targets_str
            .split_whitespace()
            .filter_map(|dist| {
                let parts: Vec<&str> = dist.split(':').collect();
                if parts.len() == 2 {
                    let symbol = parts[0].to_string();
                    let target = parts[1].parse::<f64>().ok()?;
                    Some(TokenCount { symbol, target })
                } else {
                    None
                }
            })
            .collect()
    }
    
    /// Fetch markets from Solend API
    pub async fn fetch_markets(&self) -> Result<Vec<MarketConfig>> {
        let url = self.get_markets_url();
        
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        let mut attempts = 0;
        let max_attempts = 10;
        let mut backoff = Duration::from_millis(10);
        
        loop {
            attempts += 1;
            
            match client.get(&url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        let markets: Vec<MarketConfig> = response.json().await?;
                        log::info!("Fetched {} markets from Solend API", markets.len());
                        return Ok(markets);
                    } else {
                        log::error!("Failed to fetch markets: HTTP {}", response.status());
                    }
                }
                Err(e) => {
                    log::error!("Error fetching markets (attempt {}/{}): {}", attempts, max_attempts, e);
                }
            }
            
            if attempts >= max_attempts {
                return Err(anyhow!("Failed to fetch markets after {} attempts", max_attempts));
            }
            
            tokio::time::sleep(backoff).await;
            backoff *= 2;
        }
    }
    
    /// Get markets API URL based on configuration
    fn get_markets_url(&self) -> String {
        if let Some(ref market_ids) = self.markets_filter {
            format!("https://api.solend.fi/v1/markets/configs?ids={}", market_ids)
        } else {
            format!(
                "https://api.solend.fi/v1/markets/configs?scope=solend&deployment={}",
                self.app
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_targets() {
        let targets = Config::parse_targets("USDC:100 USDT:5 SOL:0.5");
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0].symbol, "USDC");
        assert_eq!(targets[0].target, 100.0);
        assert_eq!(targets[2].symbol, "SOL");
        assert_eq!(targets[2].target, 0.5);
    }
}
