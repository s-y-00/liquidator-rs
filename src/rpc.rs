use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    account::Account,
};
use std::str::FromStr;

use crate::models::{Obligation, Reserve};
use crate::models::obligation::OBLIGATION_SIZE;
use crate::models::reserve::RESERVE_SIZE;
use crate::utils::get_program_id;

/// RPC client wrapper with convenience methods
pub struct SolendRpcClient {
    client: RpcClient,
    program_id: Pubkey,
}

impl SolendRpcClient {
    /// Create new RPC client
    pub fn new(rpc_endpoint: &str, env: &str) -> Result<Self> {
        let client = RpcClient::new_with_commitment(
            rpc_endpoint.to_string(),
            CommitmentConfig::confirmed(),
        );
        
        let program_id = get_program_id(env)?;
        
        Ok(Self { client, program_id })
    }
    
    /// Fetch all obligations for a lending market
    pub fn get_obligations(&self, lending_market_addr: &str) -> Result<Vec<(Pubkey, Obligation)>> {
        use solana_client::rpc_filter::{RpcFilterType, Memcmp, MemcmpEncodedBytes};
        
        let _market_pubkey = Pubkey::from_str(lending_market_addr)?;
        
        let filters = vec![
            // Filter by lending market address at offset 10
            RpcFilterType::Memcmp(Memcmp::new(
                10,
                MemcmpEncodedBytes::Base58(lending_market_addr.to_string()),
            )),
            // Filter by data size
            RpcFilterType::DataSize(OBLIGATION_SIZE as u64),
        ];
        
        let accounts = self.client
            .get_program_accounts_with_config(
                &self.program_id,
                solana_client::rpc_config::RpcProgramAccountsConfig {
                    filters: Some(filters),
                    account_config: solana_client::rpc_config::RpcAccountInfoConfig {
                        encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                        commitment: Some(CommitmentConfig::confirmed()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )?;
        
        let mut obligations = Vec::new();
        
        for (pubkey, account) in accounts {
            match Obligation::parse(&account.data) {
                Ok(obligation) => {
                    if !obligation.last_update.is_zero() {
                        obligations.push((pubkey, obligation));
                    }
                }
                Err(e) => {
                    log::warn!("Failed to parse obligation {}: {}", pubkey, e);
                }
            }
        }
        
        log::info!("Fetched {} obligations for market {}", obligations.len(), lending_market_addr);
        Ok(obligations)
    }
    
    /// Fetch all reserves for a lending market
    pub fn get_reserves(&self, lending_market_addr: &str) -> Result<Vec<(Pubkey, Reserve)>> {
        use solana_client::rpc_filter::{RpcFilterType, Memcmp, MemcmpEncodedBytes};
        
        let filters = vec![
            // Filter by lending market address at offset 10
            RpcFilterType::Memcmp(Memcmp::new(
                10,
                MemcmpEncodedBytes::Base58(lending_market_addr.to_string()),
            )),
            // Filter by data size
            RpcFilterType::DataSize(RESERVE_SIZE as u64),
        ];
        
        let accounts = self.client
            .get_program_accounts_with_config(
                &self.program_id,
                solana_client::rpc_config::RpcProgramAccountsConfig {
                    filters: Some(filters),
                    account_config: solana_client::rpc_config::RpcAccountInfoConfig {
                        encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                        commitment: Some(CommitmentConfig::confirmed()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )?;
        
        let mut reserves = Vec::new();
        
        for (pubkey, account) in accounts {
            match Reserve::parse(&account.data) {
                Ok(reserve) => {
                    if !reserve.last_update.is_zero() {
                        reserves.push((pubkey, reserve));
                    }
                }
                Err(e) => {
                    log::warn!("Failed to parse reserve {}: {}", pubkey, e);
                }
            }
        }
        
        log::info!("Fetched {} reserves for market {}", reserves.len(), lending_market_addr);
        Ok(reserves)
    }
    
    /// Get account info
    pub fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        self.client
            .get_account(pubkey)
            .map_err(|e| anyhow!("Failed to get account {}: {}", pubkey, e))
    }
    
    /// Get multiple accounts
    pub fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        self.client
            .get_multiple_accounts(pubkeys)
            .map_err(|e| anyhow!("Failed to get multiple accounts: {}", e))
    }
    
    /// Get inner client reference
    pub fn client(&self) -> &RpcClient {
        &self.client
    }
}
