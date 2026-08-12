// SPDX-License-Identifier: LicenseRef-Blockscout

use serde::Deserialize;
use serde_with::serde_as;
use std::time::Duration;

use crate::{
    avalanche_data_api::AvalancheDataApiClientSettings,
    indexer::failure_ledger::settings::FailureRetrySettings,
};

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
    pub data_api_client_settings: AvalancheDataApiClientSettings,
    #[serde(default)]
    pub failure_retry: FailureRetrySettings,
}

impl Default for AvalancheIndexerSettings {
    fn default() -> Self {
        Self {
            pull_interval_ms: default_pull_interval(),
            batch_size: default_batch_size(),
            data_api_client_settings: AvalancheDataApiClientSettings::default(),
            failure_retry: FailureRetrySettings::default(),
        }
    }
}

fn default_pull_interval() -> Duration {
    Duration::from_millis(10_000)
}

fn default_batch_size() -> u64 {
    1000
}
