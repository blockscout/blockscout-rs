use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub metrics: MetricsSettings,
    #[serde(default)]
    pub tracing: TracingSettings,
    #[serde(default)]
    pub jaeger: JaegerSettings,

    pub database: DatabaseSettings,

    pub api: ApiSettings,

    pub indexer: IndexerSettings,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "USER_OPS_INDEXER";
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApiSettings {
    pub max_page_size: u32,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    pub rpc_url: String,

    pub concurrency: u32,

    pub entrypoints: EntrypointsSettings,

    pub realtime: RealtimeIndexerSettings,

    pub past_rpc_logs_indexer: PastRpcLogsIndexerSettings,

    pub past_db_logs_indexer: PastDbLogsIndexerSettings,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntrypointsSettings {
    pub v06: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RealtimeIndexerSettings {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PastRpcLogsIndexerSettings {
    pub enabled: bool,

    pub block_range: u32,
}

impl PastRpcLogsIndexerSettings {
    pub fn get_block_range(&self) -> u32 {
        if self.enabled {
            self.block_range
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PastDbLogsIndexerSettings {
    pub enabled: bool,

    pub start_block: i32,

    pub end_block: i32,
}

impl PastDbLogsIndexerSettings {
    pub fn get_start_block(&self) -> i32 {
        if self.enabled {
            self.start_block
        } else {
            0
        }
    }

    pub fn get_end_block(&self) -> i32 {
        if self.enabled {
            self.end_block
        } else {
            0
        }
    }
}

impl Settings {
    pub fn default(database_url: String) -> Self {
        Self {
            server: Default::default(),
            metrics: Default::default(),
            tracing: Default::default(),
            jaeger: Default::default(),
            database: DatabaseSettings {
                connect: DatabaseConnectSettings::Url(database_url),
                create_database: false,
                run_migrations: false,
            },
            api: ApiSettings { max_page_size: 100 },
            indexer: IndexerSettings {
                rpc_url: "ws://127.0.0.1:8546".to_string(),
                concurrency: 10,
                entrypoints: EntrypointsSettings { v06: true },
                realtime: RealtimeIndexerSettings { enabled: true },
                past_rpc_logs_indexer: PastRpcLogsIndexerSettings {
                    enabled: false,
                    block_range: 0,
                },
                past_db_logs_indexer: PastDbLogsIndexerSettings {
                    enabled: false,
                    start_block: 0,
                    end_block: 0,
                },
            },
        }
    }
}
