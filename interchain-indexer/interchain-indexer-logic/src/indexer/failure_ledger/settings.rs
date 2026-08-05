// SPDX-License-Identifier: LicenseRef-Blockscout

use std::time::Duration;

use anyhow::{Result, ensure};
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
    /// across all due intervals. The pass is awaited inside the driver's
    /// `select!` loop, so for its whole duration neither the catch-up nor the
    /// realtime stream is polled — bounding that pause is the entire purpose
    /// of this knob.
    ///
    /// It bounds *chunks*, not time, so the pause is this value times the
    /// per-chunk `eth_getLogs` latency. That distinction is why the default
    /// is low: against a slow public endpoint 16 chunks measured ~87s, which
    /// is longer than `scan_interval`, so the next tick was already due when
    /// the pass returned and the forward streams made no progress between two
    /// back-to-back passes. A low value costs replay throughput and never
    /// coverage — the sweep's resume cursor continues the backlog on the next
    /// tick.
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

impl FailureRetrySettings {
    /// Rejects zero values that would either panic the indexer task or
    /// silently disable the only recovery path, instead of accepting them
    /// and clamping/defaulting quietly. Call once at indexer construction so
    /// a misconfiguration fails loudly at startup — this repo's convention
    /// for config typos (`deny_unknown_fields` everywhere).
    ///
    /// - `scan_interval == 0` is fed straight into `tokio::time::interval`
    ///   (`range_driver.rs`), which panics on a zero period — inside a
    ///   `tokio::spawn`ed indexer task, regardless of `enabled`, since the
    ///   interval is constructed unconditionally before the loop checks the
    ///   kill switch.
    /// - `max_chunks_per_pass == 0` makes every retry tick attempt zero
    ///   chunks, silently turning off the only recovery path for a recorded
    ///   hole while `enabled` still reads `true`.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.scan_interval.is_zero(),
            "failure_retry.scan_interval must be greater than zero: \
             tokio::time::interval panics on a zero period"
        );
        ensure!(
            self.max_chunks_per_pass > 0,
            "failure_retry.max_chunks_per_pass must be greater than zero, or the retry pass \
             — the only recovery path for a recorded processing failure — silently attempts \
             nothing every tick while `enabled` stays true"
        );
        Ok(())
    }
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
    2
}

fn default_record_retry_attempts() -> u32 {
    3
}

fn default_record_retry_initial_backoff() -> Duration {
    Duration::from_millis(200)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_accepts_defaults() {
        assert!(FailureRetrySettings::default().validate().is_ok());
    }

    #[test]
    fn test_validate_rejects_zero_scan_interval() {
        let settings = FailureRetrySettings {
            scan_interval: Duration::ZERO,
            ..Default::default()
        };

        let err = settings
            .validate()
            .expect_err("a zero scan_interval must be rejected");
        assert!(format!("{err:#}").contains("scan_interval"));
    }

    #[test]
    fn test_validate_rejects_zero_max_chunks_per_pass() {
        let settings = FailureRetrySettings {
            max_chunks_per_pass: 0,
            ..Default::default()
        };

        let err = settings
            .validate()
            .expect_err("a zero max_chunks_per_pass must be rejected");
        assert!(format!("{err:#}").contains("max_chunks_per_pass"));
    }

    #[test]
    fn test_validate_accepts_boundary_value_of_one() {
        let settings = FailureRetrySettings {
            scan_interval: Duration::from_secs(1),
            max_chunks_per_pass: 1,
            ..Default::default()
        };

        assert!(settings.validate().is_ok());
    }
}
