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
    /// Maximum concurrent receipt and block fetches per Avalanche batch.
    /// Mirrors AMB's setting of the same name; it was hardcoded at 25 here.
    ///
    /// Note this does not set throughput: that is bounded by the node's
    /// `max_rps`, and any excess concurrency merely parks futures against the
    /// limiter. Lower it to reduce in-flight requests on a chain you have
    /// deliberately throttled.
    #[serde(default = "default_receipt_concurrency")]
    pub receipt_concurrency: u64,
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
            receipt_concurrency: default_receipt_concurrency(),
            data_api_client_settings: AvalancheDataApiClientSettings::default(),
            failure_retry: FailureRetrySettings::default(),
        }
    }
}

/// Matches AMB's default. The two EVM adapters had a 20x difference here for
/// no reason, and this interval is a flat sleep after *every* batch in both
/// sub-streams — including catch-up, where it is pure idle time. At 10s and
/// `batch_size = 1000` it capped catch-up at 100 blocks/s per stream no matter
/// what the endpoint could serve.
fn default_pull_interval() -> Duration {
    Duration::from_millis(500)
}

fn default_batch_size() -> u64 {
    1000
}

fn default_receipt_concurrency() -> u64 {
    25
}
