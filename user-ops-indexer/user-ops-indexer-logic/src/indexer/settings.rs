use crate::indexer::rpc_utils::TraceClient;
use alloy::primitives::{address, Address};
use serde::Deserialize;
use serde_with::serde_as;
use std::time;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    pub rpc_url: String,

    pub concurrency: u32,

    pub entrypoints: EntrypointsSettings,

    pub realtime: RealtimeIndexerSettings,

    pub past_rpc_logs_indexer: PastRpcLogsIndexerSettings,

    pub past_db_logs_indexer: PastDbLogsIndexerSettings,

    #[serde(default = "default_deduplication_cache_size")]
    pub deduplication_cache_size: usize,

    #[serde(default = "default_deduplication_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub deduplication_interval: time::Duration,

    #[serde(default = "default_restart_delay")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub restart_delay: time::Duration,

    pub trace_client: Option<TraceClient>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct EntrypointsSettings {
    pub v06: bool,
    pub v06_entry_point: Address,
    pub v07: bool,
    pub v07_entry_point: Address,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealtimeIndexerSettings {
    pub enabled: bool,

    #[serde(default = "default_polling_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub polling_interval: time::Duration,

    #[serde(default = "default_polling_block_range")]
    pub polling_block_range: u32,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PastRpcLogsIndexerSettings {
    pub enabled: bool,

    pub block_range: u32,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PastDbLogsIndexerSettings {
    pub enabled: bool,

    pub start_block: i32,

    pub end_block: i32,
}

fn default_polling_interval() -> time::Duration {
    time::Duration::from_secs(6)
}

fn default_polling_block_range() -> u32 {
    6
}

fn default_deduplication_cache_size() -> usize {
    1000
}

fn default_deduplication_interval() -> time::Duration {
    time::Duration::from_secs(600)
}

fn default_restart_delay() -> time::Duration {
    time::Duration::from_secs(60)
}

impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            rpc_url: "ws://127.0.0.1:8546".to_string(),
            concurrency: 10,
            entrypoints: Default::default(),
            realtime: RealtimeIndexerSettings {
                enabled: true,
                polling_interval: default_polling_interval(),
                polling_block_range: default_polling_block_range(),
            },
            past_rpc_logs_indexer: PastRpcLogsIndexerSettings {
                enabled: false,
                block_range: 0,
            },
            past_db_logs_indexer: PastDbLogsIndexerSettings {
                enabled: false,
                start_block: 0,
                end_block: 0,
            },
            deduplication_cache_size: default_deduplication_cache_size(),
            deduplication_interval: default_deduplication_interval(),
            restart_delay: default_restart_delay(),
            trace_client: None,
        }
    }
}

impl Default for EntrypointsSettings {
    fn default() -> Self {
        Self {
            v06: true,
            v06_entry_point: address!("5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"),
            v07: true,
            v07_entry_point: address!("0000000071727De22E5E9d8BAf0edAc6f37da032"),
        }
    }
}
