use std::str::FromStr;

use ethers::{types::Address, utils::to_checksum};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Encoding {
    CheckSummedHex,
}

impl Encoding {
    pub fn encode(&self, address: &str) -> Result<String, anyhow::Error> {
        match self {
            Self::CheckSummedHex => {
                let address = Address::from_str(address)?;
                Ok(to_checksum(&address, None))
            }
        }
    }
}
