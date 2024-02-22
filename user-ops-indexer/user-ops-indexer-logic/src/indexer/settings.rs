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

    pub deduplication_cache_size: usize,

    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub deduplication_interval: time::Duration,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntrypointsSettings {
    pub v06: bool,
    pub v07: bool,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealtimeIndexerSettings {
    pub enabled: bool,

    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub polling_interval: time::Duration,

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

impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            rpc_url: "ws://127.0.0.1:8546".to_string(),
            concurrency: 10,
            entrypoints: EntrypointsSettings {
                v06: true,
                v07: true,
            },
            realtime: RealtimeIndexerSettings {
                enabled: true,
                polling_interval: time::Duration::from_secs(12),
                polling_block_range: 6,
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
            deduplication_cache_size: 1000,
            deduplication_interval: time::Duration::from_secs(600),
        }
    }
}
