use serde::Deserialize;
use serde_with::serde_as;
use std::time::Duration;

use crate::avalanche_data_api::AvalancheDataApiClientSettings;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AvalancheIndexerSettings {
    #[serde(default = "default_pull_interval")]
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub pull_interval_ms: Duration,
    #[serde(default = "default_batch_size")]
    pub batch_size: u64,
    #[serde(default)]
    pub process_unknown_chains: bool,
    #[serde(default)]
    pub data_api_client_settings: AvalancheDataApiClientSettings,
}

impl Default for AvalancheIndexerSettings {
    fn default() -> Self {
        Self {
            pull_interval_ms: default_pull_interval(),
            batch_size: default_batch_size(),
            process_unknown_chains: false,
            data_api_client_settings: AvalancheDataApiClientSettings::default(),
        }
    }
}

fn default_pull_interval() -> Duration {
    Duration::from_millis(10_000)
}

fn default_batch_size() -> u64 {
    1000
}
