use crate::logic::{config::macros, ConfigError, ConfigValidationContext};
use fang::{Deserialize, Serialize};
use serde_plain::derive_display_from_serialize;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainName(String);
derive_display_from_serialize!(ChainName);

macros::custom_env_var!(
    ChainName,
    String,
    [
        (ConfigPath, "config.network.name"),
        (ConfigPath, "config.network.shortname")
    ],
    {
        fn new(v: String, _config: &ConfigValidationContext) -> Result<Self, ConfigError> {
            Ok(Self(v))
        }
    }
);
