use anyhow::{anyhow, Result};
use clap::Parser;
use log::{error, info, warn};
use solana_sdk::signature::{read_keypair_file, Signer};
use std::str::FromStr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use futures::future::join_all;

mod config;
mod liquidation;
mod models;
mod oracle;
mod rpc;
mod utils;
mod wallet;
mod metrics;
mod cache;

use config::Config;
use liquidation::{calculate_refreshed_obligation, liquidate_and_redeem};
use rpc::SolendRpcClient;
use wallet::get_wallet_token_balance;

/// Solend Liquidator Bot - Rust Edition
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run in dry-run mode (no transactions will be submitted)
    #[arg(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let args = Args::parse();
    
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();
    
    info!("Starting Solend Liquidator Bot (Rust)");
    
    if args.dry_run {
        warn!("⚠️  DRY-RUN MODE ENABLED - No transactions will be submitted ⚠️");
    }
    
    // Load configuration
    let config = Config::from_env()?;
    
    // Validate RPC endpoint
    if config.rpc_endpoint.is_empty() {
        return Err(anyhow!("Please provide a private RPC endpoint in .env"));
    }
    
    // Fetch markets
    let markets = config.fetch_markets().await?;
    
    // Initialize RPC client
    let rpc_client = Arc::new(SolendRpcClient::new(&config.rpc_endpoint, &config.app)?);
    
    // Load wallet keypair
    let payer = Arc::new(read_keypair_file(&config.secret_path)
        .map_err(|e| anyhow!("Failed to read keypair from {}: {}", config.secret_path, e))?);
    
    let config_arc = Arc::new(config.clone()); // Clone config for sharing (it's cheap if fields are strings)
    
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
    
    // Pre-build token mints cache for all markets (optimization)
    info!("Building token mints cache...");
    let mut token_mints_cache: HashMap<String, HashMap<String, (solana_sdk::pubkey::Pubkey, u8)>> = HashMap::new();
    for market in &markets {
        let mut mints = HashMap::new();
        for reserve in &market.reserves {
            if let Ok(mint) = solana_sdk::pubkey::Pubkey::from_str(&reserve.liquidity_token.mint) {
        mints.insert(
                    reserve.liquidity_token.symbol.clone(),
                    (mint, reserve.decimals()),
                );
            }
        }
        token_mints_cache.insert(market.address.clone(), mints);
    }
    let token_mints_cache = Arc::new(token_mints_cache);
    info!("Token mints cache built for {} markets", token_mints_cache.len());
    
    // Semaphore to limit concurrent market processing
    // Use a reasonable limit (e.g., 10) to avoid open file limits or overwhelming RPC
    let semaphore = Arc::new(Semaphore::new(10));
    let args_arc = Arc::new(args);
    
    // Main liquidation loop
    let mut epoch = 0u64;
    
    loop {
        epoch += 1;
        let mut overall_metrics = metrics::PerformanceMetrics::start_epoch();
        
        let mut tasks = Vec::new();
        
        for market in markets.clone() { // Clone market config for each task
            let rpc_client = rpc_client.clone();
            let payer = payer.clone();
            let config = config_arc.clone();
            let args = args_arc.clone();
            let token_mints_cache = token_mints_cache.clone();
            let semaphore = semaphore.clone();
            
            tasks.push(tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                process_market(
                    rpc_client,
                    config,
                    payer,
                    args,
                    market,
                    token_mints_cache,
                ).await
            }));
        }
        
        // Wait for all markets to be processed
        let results = join_all(tasks).await;
        
        // Aggregate metrics
        for result in results {
            match result {
                Ok(Ok(metrics)) => {
                    // Manually sum up metrics
                    overall_metrics.oracle_fetch_ms += metrics.oracle_fetch_ms;
                    overall_metrics.obligations_fetch_ms += metrics.obligations_fetch_ms;
                    overall_metrics.reserves_fetch_ms += metrics.reserves_fetch_ms;
                    overall_metrics.processing_ms += metrics.processing_ms;
                    overall_metrics.total_obligations += metrics.total_obligations;
                    overall_metrics.unhealthy_obligations += metrics.unhealthy_obligations;
                    overall_metrics.liquidations_attempted += metrics.liquidations_attempted;
                }
                Ok(Err(e)) => {
                    error!("Market processing failed: {}", e);
                }
                Err(e) => {
                    error!("Task join error: {}", e);
                }
            }
        }
        
        // Post-processing: Unwrap and Rebalance ONCE per epoch (safer and more efficient than per market)
        
        // Unwrap wrapped tokens
        if let Err(e) = wallet::unwrap_all_wrapped_tokens(rpc_client.client(), &payer).await {
            warn!("Failed to unwrap tokens: {}", e);
        }
        
        // Rebalance wallet if targets configured
        // We use the first market's token mints for reference or merge them?
        // Actually rebalance_wallet needs a map of all token mints to check balances properly?
        // The implementation uses `token_mints` mainly for decimals lookup of target tokens.
        // We can pass a merged map or just pick one if targets are commonly available.
        // Better: Pass the full cache or create a combined map if needed.
        // For now, let's use the first available market map assuming targets are liquid tokens present in markets.
        if !config_arc.targets.is_empty() {
             // Find a market that has the target tokens? 
             // Simplification: Use the first market map found, or merge.
             if let Some(first_market_mints) = token_mints_cache.values().next() {
                 if let Err(e) = wallet::rebalance_wallet(
                    rpc_client.client(),
                    &payer,
                    &config_arc.app,
                    &config_arc.targets,
                    config_arc.rebalance_padding,
                    first_market_mints,
                ).await {
                    warn!("Failed to rebalance wallet: {}", e);
                }
             }
        }
        
        overall_metrics.log_summary();
        info!("Epoch {} complete, starting next iteration...\n", epoch);
        
        // Throttle to avoid rate limiting
        if config_arc.throttle_ms > 0 {
            utils::wait(config_arc.throttle_ms).await;
        }
    }
}

/// Process a single market: fetch data, check obligations, liquidate unhealthy ones
async fn process_market(
    rpc_client: Arc<SolendRpcClient>,
    config: Arc<Config>,
    payer: Arc<solana_sdk::signature::Keypair>,
    args: Arc<Args>,
    market: models::MarketConfig,
    token_mints_cache: Arc<HashMap<String, HashMap<String, (solana_sdk::pubkey::Pubkey, u8)>>>,
) -> Result<metrics::PerformanceMetrics> {
    let mut metrics = metrics::PerformanceMetrics::default();
    
    // info!("Checking market: {} ({})", market.name, market.address);
    
    // Fetch data in parallel
    let fetch_start = std::time::Instant::now();
    let (oracle_result, obligations_result, reserves_result) = tokio::join!(
        oracle::get_tokens_oracle_data(&rpc_client, &market),
        async { rpc_client.get_obligations(&market.address) },
        async { rpc_client.get_reserves(&market.address) }
    );
    
    // Note: Simple accumulating timing for metrics (won't be perfect in parallel)
    metrics.oracle_fetch_ms = fetch_start.elapsed().as_millis() as u64;

    let oracle_data = match oracle_result {
        Ok(data) => data,
        Err(e) => return Err(anyhow!("Failed to fetch oracle data for market {}: {}", market.name, e)),
    };
    
    let obligations = match obligations_result {
        Ok(obs) => obs,
        Err(e) => return Err(anyhow!("Failed to fetch obligations for market {}: {}", market.name, e)),
    };
    
    let reserves = match reserves_result {
        Ok(res) => res,
        Err(e) => return Err(anyhow!("Failed to fetch reserves for market {}: {}", market.name, e)),
    };
    
    let reserves_map: HashMap<solana_sdk::pubkey::Pubkey, models::Reserve> = reserves.into_iter().collect();
    
    metrics.total_obligations = obligations.len();
    
    // Filter unhealthy obligations
    let processing_start = std::time::Instant::now();
    let unhealthy_obligations: Vec<_> = obligations.iter()
        .filter_map(|(pubkey, obligation)| {
            let refreshed = calculate_refreshed_obligation(
                obligation,
                &reserves_map,
                &oracle_data,
            ).ok()?;
            
            if refreshed.is_unhealthy() {
                Some((pubkey, obligation.clone(), refreshed))
            } else {
                None
            }
        })
        .collect();
    
    if unhealthy_obligations.is_empty() {
        metrics.processing_ms = processing_start.elapsed().as_millis() as u64;
        return Ok(metrics);
    }
    
    info!("[{}] Found {} unhealthy obligations", market.name, unhealthy_obligations.len());
    metrics.unhealthy_obligations = unhealthy_obligations.len();

    // Batch fetch wallet balances
    let mut decimals_map = HashMap::new();
    let mut needed_mints = std::collections::HashSet::new();
    
    for reserve in &market.reserves {
        if let Ok(mint) = solana_sdk::pubkey::Pubkey::from_str(&reserve.liquidity_token.mint) {
            decimals_map.insert(mint, reserve.decimals());
        }
    }

    for (_, _, refreshed) in &unhealthy_obligations {
        if let Some(borrow) = refreshed.select_repay_borrow() {
            if let Ok(mint) = solana_sdk::pubkey::Pubkey::from_str(&borrow.mint_address) {
                needed_mints.insert(mint);
            }
        }
    }

    let needed_mints_vec: Vec<_> = needed_mints.into_iter().collect();
    let wallet_balances = if !needed_mints_vec.is_empty() {
        wallet::get_wallet_token_balances_batched(
            &rpc_client,
            &payer.pubkey(),
            &needed_mints_vec,
            &decimals_map,
        ).await.unwrap_or_default()
    } else {
        HashMap::new()
    };
    
    // Process liquidations
    for (obligation_pubkey, mut obligation, mut refreshed) in unhealthy_obligations {
        loop {
            if !refreshed.is_unhealthy() {
                break;
            }
            
            metrics.liquidations_attempted += 1;
            
            let selected_borrow = match refreshed.select_repay_borrow() {
                Some(b) => b,
                None => break,
            };
            
            let selected_deposit = match refreshed.select_withdraw_deposit() {
                Some(d) => d,
                None => break,
            };
            
            info!("[{}] Liquidating obl {} (borrow: {}, deposit: {})", 
                market.name, obligation_pubkey, selected_borrow.symbol, selected_deposit.symbol);
            
            let mint_pubkey = solana_sdk::pubkey::Pubkey::from_str(&selected_borrow.mint_address)?;
            
            let (balance_base, _) = if let Some((base, human)) = wallet_balances.get(&mint_pubkey) {
                (*base, *human)
            } else {
                // Fallback
                 let decimals = market
                    .find_reserve(&selected_borrow.symbol)
                    .map(|r| r.decimals())
                    .unwrap_or(9);

                get_wallet_token_balance(
                    rpc_client.client(),
                    &mint_pubkey,
                    &payer.pubkey(),
                    decimals,
                )?
            };
            
            if balance_base == 0 {
                info!("Insufficient {} balance", selected_borrow.symbol);
                break;
            }
            
            match liquidate_and_redeem(
                rpc_client.client(),
                &config.app,
                &payer, // usage of &Arc<Keypair> works as &Keypair
                balance_base,
                &selected_borrow.symbol,
                &selected_deposit.symbol,
                &market,
                &obligation,
                args.dry_run,
            ).await {
                Ok(_) => {
                    info!("Liquidation sent!");
                    // Refresh obligation logic (simplified for parallel version - might need fetch)
                     match rpc_client.client().get_account(obligation_pubkey) {
                        Ok(account) => {
                             if let Ok(updated) = models::Obligation::parse(&account.data) {
                                obligation = updated;
                                if let Ok(r) = calculate_refreshed_obligation(&obligation, &reserves_map, &oracle_data) {
                                    refreshed = r;
                                } else { break; }
                             } else { break; }
                        }
                        Err(_) => break,
                    }
                }
                Err(e) => {
                    error!("Liquidation failed: {}", e);
                    break;
                }
            }
        }
    }
    
    metrics.processing_ms = processing_start.elapsed().as_millis() as u64;
    Ok(metrics)

}
