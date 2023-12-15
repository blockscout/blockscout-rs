use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

const COIN_TYPES_JSON: &str = include_str!("coin_types.json");

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Coin {
    pub name: String,
    pub coin_type: String,
}

lazy_static! {
    pub static ref COINS_TYPES: Vec<Coin> =
        serde_json::from_str(COIN_TYPES_JSON).expect("coin_types.json should be valid");
}

pub fn coin_name(coin_type: &str) -> String {
    COINS_TYPES
        .iter()
        .find(|c| c.coin_type == coin_type)
        .map(|c| c.name.to_string())
        .unwrap_or_else(|| format!("unknown coin ({coin_type})"))
}

#[cfg(test)]
mod tests {
    use super::coin_name;

    #[test]
    fn it_works() {
        for (coin_type, expected_name) in &[
            ("0", "BTC"),
            ("60", "ETH"),
            ("137", "RSK"),
            ("2147483785", "MATIC"),
            ("9999999999", "unknown coin (9999999999)"),
        ] {
            let name = coin_name(coin_type);
            assert_eq!(name.as_str(), *expected_name)
        }
    }
}
