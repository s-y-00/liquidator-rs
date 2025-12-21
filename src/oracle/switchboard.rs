use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use crate::rpc::SolendRpcClient;

const SWITCHBOARD_V1_ADDRESS: &str = "DtmE9D2CSB4L5D6A15mraeEjrGMm6auWVzgaD8hK2tZM";
const SWITCHBOARD_V2_ADDRESS: &str = "SW1TCH7qEPTdLsDHRgPuMQjbQxKdH2aBStViMFnt64f";

/// Fetch price from Switchboard oracle
pub async fn fetch_switchboard_price(
    client: &SolendRpcClient,
    oracle_address: &str,
) -> Result<Decimal> {
    let pubkey = Pubkey::from_str(oracle_address)?;
    let account = client.get_account(&pubkey)?;
    
    let owner = account.owner.to_string();
    
    // Note: This is a simplified implementation
    // Full Switchboard V1/V2 parsing would require additional crates
    // For now, we'll return an error and recommend using Pyth oracles
    
    if owner == SWITCHBOARD_V1_ADDRESS {
        Err(anyhow!("Switchboard V1 not yet implemented - please use Pyth oracles"))
    } else if owner == SWITCHBOARD_V2_ADDRESS {
        Err(anyhow!("Switchboard V2 not yet implemented - please use Pyth oracles"))
    } else {
        Err(anyhow!("Unrecognized switchboard owner address: {}", owner))
    }
}
