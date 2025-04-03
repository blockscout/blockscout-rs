use crate::indexer::rpc_utils::TraceClient;
use alloy::primitives::{address, Address};
use serde::Deserialize;
use serde_with::{formats::CommaSeparator, serde_as};
use std::time;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct IndexerSettings {
    pub rpc_url: String,

    pub concurrency: u32,

    pub entrypoints: EntrypointsSettings,

    pub realtime: RealtimeIndexerSettings,

    pub past_rpc_logs_indexer: PastRpcLogsIndexerSettings,

    pub past_db_logs_indexer: PastDbLogsIndexerSettings,

    pub deduplication_cache_size: usize,

    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub deduplication_interval: time::Duration,

    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub restart_delay: time::Duration,

    pub trace_client: Option<TraceClient>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct EntrypointsSettings {
    pub v06: bool,
    #[serde_as(as = "serde_with::StringWithSeparator<CommaSeparator, Address>")]
    pub v06_entry_point: Vec<Address>,
    pub v07: bool,
    #[serde_as(as = "serde_with::StringWithSeparator<CommaSeparator, Address>")]
    pub v07_entry_point: Vec<Address>,
    pub v08: bool,
    #[serde_as(as = "serde_with::StringWithSeparator<CommaSeparator, Address>")]
    pub v08_entry_point: Vec<Address>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct RealtimeIndexerSettings {
    pub enabled: bool,

    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub polling_interval: time::Duration,

    pub polling_block_range: u32,

    pub max_block_range: u32,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct PastRpcLogsIndexerSettings {
    pub enabled: bool,

    pub block_range: u32,
}

#[serde_as]
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct PastDbLogsIndexerSettings {
    pub enabled: bool,

    pub start_block: i32,

    pub end_block: i32,
}

impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            rpc_url: "ws://127.0.0.1:8546".to_string(),
            concurrency: 10,
            entrypoints: Default::default(),
            realtime: Default::default(),
            past_rpc_logs_indexer: Default::default(),
            past_db_logs_indexer: Default::default(),
            deduplication_cache_size: 1000,
            deduplication_interval: time::Duration::from_secs(600),
            restart_delay: time::Duration::from_secs(60),
            trace_client: None,
        }
    }
}

impl Default for EntrypointsSettings {
    fn default() -> Self {
        Self {
            v06: true,
            v06_entry_point: vec![address!("5FF137D4b0FDCD49DcA30c7CF57E578a026d2789")],
            v07: true,
            v07_entry_point: vec![address!("0000000071727De22E5E9d8BAf0edAc6f37da032")],
            v08: true,
            v08_entry_point: vec![address!("4337084D9E255Ff0702461CF8895CE9E3b5Ff108")],
        }
    }
}

impl Default for RealtimeIndexerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            polling_interval: time::Duration::from_secs(6),
            polling_block_range: 6,
            max_block_range: 10000,
        }
    }
}
