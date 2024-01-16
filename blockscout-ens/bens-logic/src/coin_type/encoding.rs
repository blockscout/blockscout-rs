use ethers::{types::Address, utils::to_checksum};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Encoding {
    CheckSummedHex(Option<u8>),
}

impl Encoding {
    pub fn encode(&self, address: &str) -> Result<String, anyhow::Error> {
        match self {
            Self::CheckSummedHex(chain_id) => {
                let address = Address::from_str(address)?;
                Ok(to_checksum(&address, *chain_id))
            }
        }
    }
}
