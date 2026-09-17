use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::indexer::failure_ledger::settings::FailureRetrySettings;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct XDaiIndexerSettings {
    #[serde(default = "default_pull_interval")]
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub pull_interval_ms: Duration,
    #[serde(default = "default_batch_size")]
    pub batch_size: u64,
    #[serde(default = "default_receipt_concurrency")]
    pub receipt_concurrency: u64,
    #[serde(default)]
    pub failure_retry: FailureRetrySettings,
}

impl Default for XDaiIndexerSettings {
    fn default() -> Self {
        Self {
            pull_interval_ms: default_pull_interval(),
            batch_size: default_batch_size(),
            receipt_concurrency: default_receipt_concurrency(),
            failure_retry: FailureRetrySettings::default(),
        }
    }
}

fn default_pull_interval() -> Duration {
    Duration::from_millis(500)
}

fn default_batch_size() -> u64 {
    1000
}

fn default_receipt_concurrency() -> u64 {
    25
}
