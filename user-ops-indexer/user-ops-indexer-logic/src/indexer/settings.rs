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
        }
    }
}
