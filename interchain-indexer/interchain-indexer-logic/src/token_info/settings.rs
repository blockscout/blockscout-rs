use serde::Deserialize;
use serde_with::{StringWithSeparator, formats::CommaSeparator, serde_as};
use std::time::Duration;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlockscoutTokenInfoClientSettings {
    #[serde(default)]
    pub url: Option<String>,

    // Do not request icons for the tokens from chains with these IDs.
    #[serde_as(as = "StringWithSeparator<CommaSeparator, i64>")]
    #[serde(default = "default_ignore_chains")]
    pub ignore_chains: Vec<i64>,

    // If the token icon is not found in the the external token info service,
    // do not retry fetching it during this interval.
    #[serde(default = "default_icon_retry_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub retry_interval: Duration,
}

fn default_ignore_chains() -> Vec<i64> {
    vec![]
}

fn default_icon_retry_interval() -> Duration {
    Duration::from_secs(3600)
}

impl Default for BlockscoutTokenInfoClientSettings {
    fn default() -> Self {
        Self {
            url: None,
            ignore_chains: default_ignore_chains(),
            retry_interval: default_icon_retry_interval(),
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct TokenInfoServiceSettings {
    #[serde(default)]
    pub blockscout_token_info: BlockscoutTokenInfoClientSettings,

    // If the onchain request for the token info was unsuccessful,
    // do not retry fetching it during this interval.
    #[serde(default = "default_onchain_retry_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub onchain_retry_interval: Duration,
}

fn default_onchain_retry_interval() -> Duration {
    Duration::from_secs(10)
}

impl Default for TokenInfoServiceSettings {
    fn default() -> Self {
        Self {
            blockscout_token_info: Default::default(),
            onchain_retry_interval: default_onchain_retry_interval(),
        }
    }
}
