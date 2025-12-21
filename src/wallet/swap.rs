use anyhow::{anyhow, Result};
use reqwest;
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

/// Jupiter API v6 base URL
const JUPITER_API_URL: &str = "https://quote-api.jup.ag/v6";

/// Jupiter quote response
#[derive(Debug, Deserialize, Serialize)]
pub struct QuoteResponse {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,
    #[serde(rename = "swapMode")]
    pub swap_mode: String,
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: String,
}

/// Jupiter swap request
#[derive(Debug, Serialize)]
struct SwapRequest {
    #[serde(rename = "quoteResponse")]
    quote_response: QuoteResponse,
    #[serde(rename = "userPublicKey")]
    user_public_key: String,
    #[serde(rename = "wrapAndUnwrapSol")]
    wrap_and_unwrap_sol: bool,
}

/// Jupiter swap response
#[derive(Debug, Deserialize)]
struct SwapResponse {
    #[serde(rename = "swapTransaction")]
    swap_transaction: String,
}

/// Jupiter client for swap operations
pub struct JupiterClient {
    client: reqwest::Client,
    api_url: String,
}

impl JupiterClient {
    /// Create a new Jupiter client
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: JUPITER_API_URL.to_string(),
        }
    }
    
    /// Get a swap quote from Jupiter
    pub async fn get_quote(
        &self,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
        slippage_bps: u16,
    ) -> Result<QuoteResponse> {
        let url = format!(
            "{}/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}",
            self.api_url,
            input_mint,
            output_mint,
            amount,
            slippage_bps
        );
        
        log::debug!("Fetching Jupiter quote: {}", url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch Jupiter quote: {}", e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Jupiter API error ({}): {}",
                status,
                error_text
            ));
        }
        
        let quote: QuoteResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse Jupiter quote: {}", e))?;
        
        log::info!(
            "Jupiter quote: {} {} -> {} {} (impact: {}%)",
            quote.in_amount,
            input_mint,
            quote.out_amount,
            output_mint,
            quote.price_impact_pct
        );
        
        Ok(quote)
    }
    
    /// Execute a swap transaction
    pub async fn execute_swap(
        &self,
        rpc_client: &RpcClient,
        payer: &Keypair,
        quote: QuoteResponse,
    ) -> Result<Signature> {
        let swap_request = SwapRequest {
            quote_response: quote,
            user_public_key: payer.pubkey().to_string(),
            wrap_and_unwrap_sol: true,
        };
        
        let url = format!("{}/swap", self.api_url);
        
        log::debug!("Requesting Jupiter swap transaction");
        
        let response = self.client
            .post(&url)
            .json(&swap_request)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to request Jupiter swap: {}", e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Jupiter swap API error ({}): {}",
                status,
                error_text
            ));
        }
        
        let swap_response: SwapResponse = response.json().await
            .map_err(|e| anyhow!("Failed to parse Jupiter swap response: {}", e))?;
        
        // Decode the transaction
        use base64::Engine;
        let transaction_bytes = base64::engine::general_purpose::STANDARD
            .decode(&swap_response.swap_transaction)
            .map_err(|e| anyhow!("Failed to decode swap transaction: {}", e))?;
        
        let mut transaction: Transaction = bincode::deserialize(&transaction_bytes)
            .map_err(|e| anyhow!("Failed to deserialize swap transaction: {}", e))?;
        
        // Sign the transaction
        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        transaction.partial_sign(&[payer], recent_blockhash);
        
        // Send and confirm
        let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
        
        log::info!("Jupiter swap successful! Signature: {}", signature);
        
        Ok(signature)
    }
    
    /// Convenience method to quote and execute a swap
    pub async fn swap(
        &self,
        rpc_client: &RpcClient,
        payer: &Keypair,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
        slippage_bps: u16,
    ) -> Result<Signature> {
        let quote = self.get_quote(input_mint, output_mint, amount, slippage_bps).await?;
        self.execute_swap(rpc_client, payer, quote).await
    }
}

impl Default for JupiterClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Get USDC mint address for the given environment
pub fn get_usdc_mint(env: &str) -> Result<Pubkey> {
    match env {
        "production" => Ok(Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?),
        "devnet" => Ok(Pubkey::from_str("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU")?),
        _ => Err(anyhow!("Unknown environment: {}", env)),
    }
}
