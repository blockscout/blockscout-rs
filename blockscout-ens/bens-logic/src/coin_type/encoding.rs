use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Encoding {
    CheckSummedHex(Option<u64>),
}

impl Encoding {
    pub fn encode(&self, address: &str) -> Result<String, anyhow::Error> {
        match self {
            Self::CheckSummedHex(chain_id) => {
                let address = Address::from_str(address)?;
                Ok(address.to_checksum(*chain_id))
            }
        }
    }
}
