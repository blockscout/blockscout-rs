use std::thread;
use serde::Deserialize;


#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
   pub enabled: bool,
   pub concurrency: u32,
   pub polling_interval: u64,
   pub historical_batch_size: u32,
   pub status_update_batch_size: u32,
   pub realtime_fetch_batch_size: u32,
   pub retry_threshold: u32,
   pub failed_cctxs_polling_interval: u64,
   pub realtime_threshold: i64,
}


fn default_concurrency() -> u32 {
    thread::available_parallelism()
    .map(|p| p.get() as u32)
    .unwrap_or(1)
}


impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            concurrency: default_concurrency(), 
            polling_interval: 1000,
            historical_batch_size: 100,
            status_update_batch_size: 10,
            realtime_fetch_batch_size: 10,
            retry_threshold: 10,
            failed_cctxs_polling_interval: 1_000_000,
            realtime_threshold: 10000,
        }
    }
}