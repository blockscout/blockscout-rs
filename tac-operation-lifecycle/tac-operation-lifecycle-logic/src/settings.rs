use std::{thread, time};

use serde::Deserialize;
use serde_with::serde_as;
#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_start_timestamp")]
    pub start_timestamp: u64,
    #[serde(default = "default_polling_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub polling_interval: time::Duration,
    #[serde(default = "default_retry_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub retry_interval: time::Duration,
    #[serde(default = "default_catchup_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub catchup_interval: time::Duration,

    #[serde(default = "default_intervals_query_batch")]
    pub intervals_query_batch: usize,
    #[serde(default = "default_intervals_retry_batch")]
    pub intervals_retry_batch: usize,
    #[serde(default = "default_intervals_loop_delay_ms")]
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub intervals_loop_delay_ms: time::Duration,

    #[serde(default = "default_operations_query_batch")]
    pub operations_query_batch: usize,
    #[serde(default = "default_operations_retry_batch")]
    pub operations_retry_batch: usize,
    #[serde(default = "default_operations_loop_delay_ms")]
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub operations_loop_delay_ms: time::Duration,

    #[serde(default = "default_forever_pending_operations_age_sec")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub forever_pending_operations_age_sec: time::Duration,
}

fn default_concurrency() -> u32 {
    thread::available_parallelism()
        .map(|p| p.get() as u32)
        .unwrap_or(1)
}

fn default_start_timestamp() -> u64 {
    1740787200 // Mar 01 2025 00:00:00 GMT+0000
}

fn default_polling_interval() -> time::Duration {
    time::Duration::from_secs(1)
}

fn default_retry_interval() -> time::Duration {
    time::Duration::from_secs(60)
}

fn default_catchup_interval() -> time::Duration {
    time::Duration::from_secs(5)
}

fn default_intervals_query_batch() -> usize {
    10
}

fn default_intervals_retry_batch() -> usize {
    10
}

fn default_intervals_loop_delay_ms() -> time::Duration {
    time::Duration::from_millis(100)
}

fn default_operations_query_batch() -> usize {
    10
}

fn default_operations_retry_batch() -> usize {
    10
}

fn default_operations_loop_delay_ms() -> time::Duration {
    time::Duration::from_millis(200)
}

fn default_forever_pending_operations_age_sec() -> time::Duration {
    time::Duration::from_secs(60 * 60 * 24 * 7) // a week
}

impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            concurrency: default_concurrency(),
            start_timestamp: default_start_timestamp(),
            polling_interval: default_polling_interval(),
            retry_interval: default_retry_interval(),
            catchup_interval: default_catchup_interval(),
            intervals_query_batch: default_intervals_query_batch(),
            intervals_retry_batch: default_intervals_retry_batch(),
            intervals_loop_delay_ms: default_intervals_loop_delay_ms(),
            operations_query_batch: default_operations_query_batch(),
            operations_retry_batch: default_operations_retry_batch(),
            operations_loop_delay_ms: default_operations_loop_delay_ms(),
            forever_pending_operations_age_sec: default_forever_pending_operations_age_sec(),
        }
    }
}
