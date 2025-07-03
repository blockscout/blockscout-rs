use std::thread;
use serde::Deserialize;


#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
   pub concurrency: u32,
   pub polling_interval: u64,
   pub historical_batch_size: u32,
   pub status_update_batch_size: u32,
   pub realtime_fetch_batch_size: u32,
}


fn default_concurrency() -> u32 {
    thread::available_parallelism()
    .map(|p| p.get() as u32)
    .unwrap_or(1)
}


impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            concurrency: default_concurrency(), 
            polling_interval: 1000,
            historical_batch_size: 100,
            status_update_batch_size: 10,
            realtime_fetch_batch_size: 10,
        }
    }
}