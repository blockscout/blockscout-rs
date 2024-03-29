use crate::logic::config::{macros, Error};
use serde::{Deserialize, Serialize};
use serde_plain::derive_display_from_serialize;

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenSymbol(String);
derive_display_from_serialize!(TokenSymbol);

macros::custom_env_var!(
    TokenSymbol,
    String,
    [
        (config, "config.network.currency.symbol"),
        (config, "config.network.currency.name")
    ],
    Some("ETH".to_string()),
    {
        fn new(v: String) -> Result<Self, Error> {
            Ok(Self(v))
        }
    }
);
