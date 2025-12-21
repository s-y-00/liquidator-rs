use anyhow::{anyhow, Result};
use log::{error, info, warn};
use solana_sdk::signature::{read_keypair_file, Signer};
use std::str::FromStr;

mod config;
mod liquidation;
mod models;
mod oracle;
mod rpc;
mod utils;
mod wallet;

use config::Config;
use liquidation::{calculate_refreshed_obligation, liquidate_and_redeem};
use rpc::SolendRpcClient;
use wallet::get_wallet_token_balance;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();
    
    info!("Starting Solend Liquidator Bot (Rust)");
    
    // Load configuration
    let config = Config::from_env()?;
    
    // Validate RPC endpoint
    if config.rpc_endpoint.is_empty() {
        return Err(anyhow!("Please provide a private RPC endpoint in .env"));
    }
    
    // Fetch markets
    let markets = config.fetch_markets().await?;
    
    // Initialize RPC client
    let rpc_client = SolendRpcClient::new(&config.rpc_endpoint, &config.app)?;
    
    // Load wallet keypair
    let payer = read_keypair_file(&config.secret_path)
        .map_err(|e| anyhow!("Failed to read keypair from {}: {}", config.secret_path, e))?;
    
    info!("\nConfiguration:");
    info!("  app: {}", config.app);
    info!("  rpc: {}", config.rpc_endpoint);
    info!("  wallet: {}", payer.pubkey());
    info!("  auto-rebalancing: {}", if config.targets.is_empty() { "OFF" } else { "ON" });
    if !config.targets.is_empty() {
        info!("  rebalancing targets: {} tokens", config.targets.len());
    }
    info!("  Running against {} markets", markets.len());
    info!("");
    
    // Main liquidation loop
    let mut epoch = 0u64;
    
    loop {
        epoch += 1;
        info!("=== Epoch {} ===", epoch);
        
        for market in &markets {
            info!("Checking market: {} ({})", market.name, market.address);
            
            // Fetch oracle data
            let oracle_data = match oracle::get_tokens_oracle_data(&rpc_client, market).await {
                Ok(data) => data,
                Err(e) => {
                    error!("Failed to fetch oracle data for market {}: {}", market.name, e);
                    continue;
                }
            };
            
            // Fetch obligations
            let obligations = match rpc_client.get_obligations(&market.address) {
                Ok(obs) => obs,
                Err(e) => {
                    error!("Failed to fetch obligations for market {}: {}", market.name, e);
                    continue;
                }
            };
            
            // Fetch reserves
            let reserves = match rpc_client.get_reserves(&market.address) {
                Ok(res) => res,
                Err(e) => {
                    error!("Failed to fetch reserves for market {}: {}", market.name, e);
                    continue;
                }
            };
            
            // Convert reserves to string-keyed map for lookup
            let reserves_map: Vec<(String, models::Reserve)> = reserves
                .into_iter()
                .map(|(pubkey, reserve)| (pubkey.to_string(), reserve))
                .collect();
            
            info!("Found {} obligations to check", obligations.len());
            
            // Check each obligation for liquidation opportunity
            for (obligation_pubkey, mut obligation) in obligations {
                loop {
                    // Calculate refreshed obligation health
                    let refreshed = match calculate_refreshed_obligation(
                        &obligation,
                        &reserves_map,
                        &oracle_data,
                    ) {
                        Ok(r) => r,
                        Err(e) => {
                            warn!("Failed to calculate obligation health for {}: {}", obligation_pubkey, e);
                            break;
                        }
                    };
                    
                    // Check if obligation is healthy
                    if !refreshed.is_unhealthy() {
                        break;
                    }
                    
                    info!(
                        "Obligation {} is underwater! borrowed: {:.2}, unhealthy_threshold: {:.2}",
                        obligation_pubkey,
                        refreshed.borrowed_value,
                        refreshed.unhealthy_borrow_value
                    );
                    
                    // Select tokens to liquidate
                    let selected_borrow = match refreshed.select_repay_borrow() {
                        Some(b) => b,
                        None => {
                            warn!("No valid borrow found for obligation {}", obligation_pubkey);
                            break;
                        }
                    };
                    
                    let selected_deposit = match refreshed.select_withdraw_deposit() {
                        Some(d) => d,
                        None => {
                            warn!("No valid deposit found for obligation {}", obligation_pubkey);
                            break;
                        }
                    };
                    
                    info!(
                        "  Repaying: {} (value: {:.2})",
                        selected_borrow.symbol,
                        selected_borrow.market_value
                    );
                    info!(
                        "  Withdrawing: {} (value: {:.2})",
                        selected_deposit.symbol,
                        selected_deposit.market_value
                    );
                    
                    // Check wallet balance for repay token
                    let mint_pubkey = solana_sdk::pubkey::Pubkey::from_str(&selected_borrow.mint_address)?;
                    let decimals = market
                        .find_reserve(&selected_borrow.symbol)
                        .map(|r| r.decimals())
                        .unwrap_or(9);
                    
                    let (balance_base, balance_human) = get_wallet_token_balance(
                        rpc_client.client(),
                        &mint_pubkey,
                        &payer.pubkey(),
                        decimals,
                    )?;
                    
                    if balance_base == 0 {
                        info!(
                            "Insufficient {} to liquidate obligation {} in market: {}",
                            selected_borrow.symbol,
                            obligation_pubkey,
                            market.address
                        );
                        break;
                    }
                    
                    info!(
                        "  Wallet balance: {} {} ({} base units)",
                        balance_human,
                        selected_borrow.symbol,
                        balance_base
                    );
                    
                    // Execute liquidation
                    match liquidate_and_redeem(
                        rpc_client.client(),
                        &config.app,
                        &payer,
                        balance_base,
                        &selected_borrow.symbol,
                        &selected_deposit.symbol,
                        market,
                        &obligation,
                    ).await {
                        Ok(_) => {
                            info!("Successfully liquidated obligation {}", obligation_pubkey);
                            
                            // Fetch updated obligation to check if more liquidation needed
                            match rpc_client.client().get_account(&obligation_pubkey) {
                                Ok(account) => {
                                    match models::Obligation::parse(&account.data) {
                                        Ok(updated_obligation) => {
                                            obligation = updated_obligation;
                                            // Continue loop to check if still unhealthy
                                        }
                                        Err(_) => break,
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        Err(e) => {
                            error!("Failed to liquidate obligation {}: {}", obligation_pubkey, e);
                            break;
                        }
                    }
                }
            }
            
            // TODO: Implement token unwrapping
            // TODO: Implement wallet rebalancing if config.targets is not empty
            
            // Throttle to avoid rate limiting
            if config.throttle_ms > 0 {
                utils::wait(config.throttle_ms).await;
            }
        }
        
        info!("Epoch {} complete, starting next iteration...\n", epoch);
    }
}
