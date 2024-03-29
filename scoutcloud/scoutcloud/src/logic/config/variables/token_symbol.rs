use crate::logic::{
    config::{macros, Error},
    ConfigValidationContext,
};
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
    {
        fn new(v: String, _config: &ConfigValidationContext) -> Result<Self, Error> {
            Ok(Self(v))
        }

        fn maybe_default(_context: &ConfigValidationContext) -> Option<String> {
            Some("ETH".to_string())
        }
    }
);
