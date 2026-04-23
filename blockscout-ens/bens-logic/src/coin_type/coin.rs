use super::encoding::Encoding;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

const COIN_TYPES_JSON: &str = include_str!("coin_types.json");

lazy_static! {
    pub static ref COINS_TYPES: Vec<Coin> =
        serde_json::from_str(COIN_TYPES_JSON).expect("coin_types.json should be valid");
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Coin {
    pub name: String,
    pub coin_type: String,
    pub encoding: Option<Encoding>,
}

impl Coin {
    pub fn unknown_type(coin_type: String) -> Self {
        Self {
            name: format!("unknown coin ({coin_type})"),
            coin_type,
            encoding: None,
        }
    }

    pub fn find_or_unknown(coin_type: &str) -> Self {
        COINS_TYPES
            .iter()
            .find(|c| c.coin_type == coin_type)
            .cloned()
            .unwrap_or_else(|| Coin::unknown_type(coin_type.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        for (coin_type, expected_name) in &[
            ("0", "BTC"),
            ("60", "ETH"),
            ("137", "RSK"),
            ("2147483785", "MATIC"),
            ("9999999999", "unknown coin (9999999999)"),
        ] {
            let maybe_coin = Coin::find_or_unknown(coin_type);
            assert_eq!(maybe_coin.name.to_string(), expected_name.to_string())
        }
    }
}
