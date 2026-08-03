// SPDX-License-Identifier: LicenseRef-Blockscout

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// Configuration for the shared failed-range replay pass (`RangeDriver`).
///
/// Deliberately per-indexer, not global: this settings block only controls
/// timing/bounds. `FailureLedger`/`indexer_failures` store `attempts`,
/// `reason`, `created_at` and `updated_at`, and interpret none of them.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct FailureRetrySettings {
    /// Kill switch for the replay pass. Recording still happens when
    /// `false` — only the retry tick that re-scans open holes is paused.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// How often the retry tick fires to scan for due intervals.
    #[serde(default = "default_scan_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub scan_interval: Duration,
    /// Base delay of the capped exponential backoff (`policy::is_due`).
    #[serde(default = "default_backoff_base")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub backoff_base: Duration,
    /// Ceiling of the capped exponential backoff. This is what makes
    /// "retry forever" affordable for a permanently unrecoverable interval.
    #[serde(default = "default_backoff_cap")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub backoff_cap: Duration,
    /// Maximum number of `batch_size`-sized chunks replayed per retry tick,
    /// across all due intervals. Bounds how much a large hole set can
    /// back-pressure the realtime scan.
    #[serde(default = "default_max_chunks_per_pass")]
    pub max_chunks_per_pass: usize,
    /// Number of attempts `FailureLedger::record` makes before the driver
    /// escalates (stops consuming the stream, indexer state becomes
    /// `Failed`).
    #[serde(default = "default_record_retry_attempts")]
    pub record_retry_attempts: u32,
    /// Initial backoff between `record` retry attempts; doubles on each
    /// subsequent retry.
    #[serde(default = "default_record_retry_initial_backoff")]
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub record_retry_initial_backoff: Duration,
}

impl Default for FailureRetrySettings {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            scan_interval: default_scan_interval(),
            backoff_base: default_backoff_base(),
            backoff_cap: default_backoff_cap(),
            max_chunks_per_pass: default_max_chunks_per_pass(),
            record_retry_attempts: default_record_retry_attempts(),
            record_retry_initial_backoff: default_record_retry_initial_backoff(),
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_scan_interval() -> Duration {
    Duration::from_secs(60)
}

fn default_backoff_base() -> Duration {
    Duration::from_secs(30)
}

fn default_backoff_cap() -> Duration {
    Duration::from_secs(3600)
}

fn default_max_chunks_per_pass() -> usize {
    16
}

fn default_record_retry_attempts() -> u32 {
    3
}

fn default_record_retry_initial_backoff() -> Duration {
    Duration::from_millis(200)
}
