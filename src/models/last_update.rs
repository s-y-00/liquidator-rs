use borsh::{BorshDeserialize, BorshSerialize};

/// Last update timestamp for obligation/reserve
#[derive(Debug, Clone, Copy, BorshSerialize, BorshDeserialize)]
pub struct LastUpdate {
    pub slot: u64,
    pub stale: bool,
}

impl LastUpdate {
    pub fn is_zero(&self) -> bool {
        self.slot == 0
    }
}
