use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AmbIndexerSettings {
    #[serde(default = "default_pull_interval")]
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub pull_interval_ms: Duration,
    #[serde(default = "default_batch_size")]
    pub batch_size: u64,
    #[serde(default = "default_receipt_concurrency")]
    pub receipt_concurrency: u64,
    /// Tolerance for a destination execution preceding its source request in
    /// time. A destination timestamp earlier than the source by more than this
    /// is treated as an impossible ordering and flags an AMB `messageId`
    /// collision. Absorbs cross-chain clock skew and near-simultaneous testnet
    /// pairs while still catching genuine collisions (which differ by ~months).
    #[serde(default = "default_clock_skew_tolerance")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub clock_skew_tolerance: Duration,
}

impl Default for AmbIndexerSettings {
    fn default() -> Self {
        Self {
            pull_interval_ms: default_pull_interval(),
            batch_size: default_batch_size(),
            receipt_concurrency: default_receipt_concurrency(),
            clock_skew_tolerance: default_clock_skew_tolerance(),
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

fn default_clock_skew_tolerance() -> Duration {
    Duration::from_secs(300)
}
