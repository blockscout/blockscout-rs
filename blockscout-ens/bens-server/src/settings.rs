use blockscout_service_launcher::{
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use ethers::types::Bytes;
use serde::Deserialize;
use std::collections::HashMap;
use url::Url;

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
    #[serde(default)]
    pub subgraphs_reader: SubgraphsReaderSettings,
    pub database: DatabaseSettings,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "BENS";
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SubgraphsReaderSettings {
    pub networks: HashMap<i64, NetworkSettings>,
    #[serde(default = "default_refresh_cache_schedule")]
    pub refresh_cache_schedule: String,
    #[serde(default = "default_cache_enabled")]
    pub cache_enabled: bool,
}

fn default_cache_enabled() -> bool {
    true
}

fn default_refresh_cache_schedule() -> String {
    "0 0 * * * *".to_string() // every hour
}

impl Default for SubgraphsReaderSettings {
    fn default() -> Self {
        Self {
            networks: Default::default(),
            refresh_cache_schedule: default_refresh_cache_schedule(),
            cache_enabled: default_cache_enabled(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct NetworkSettings {
    pub blockscout: BlockscoutSettings,
    pub subgraphs: HashMap<String, SubgraphSettings>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SubgraphSettings {
    #[serde(default = "default_use_cache")]
    pub use_cache: bool,
    pub empty_label_hash: Option<Bytes>,
}

fn default_use_cache() -> bool {
    true
}

impl From<SubgraphSettings> for bens_logic::subgraphs_reader::SubgraphSettings {
    fn from(value: SubgraphSettings) -> Self {
        Self {
            use_cache: value.use_cache,
            empty_label_hash: value.empty_label_hash,
        }
    }
}

// TODO: move database settings to blockscout-service-launcher
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseSettings {
    pub connect: DatabaseConnectSettings,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
pub enum DatabaseConnectSettings {
    Url(String),
    Kv(DatabaseKvConnection),
}

impl DatabaseConnectSettings {
    pub fn url(self) -> String {
        match self {
            DatabaseConnectSettings::Url(s) => s,
            DatabaseConnectSettings::Kv(kv) => kv.url(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseKvConnection {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    #[serde(default)]
    pub dbname: Option<String>,
    #[serde(default)]
    pub options: Option<String>,
}

impl DatabaseKvConnection {
    pub fn url(self) -> String {
        let dbname = self
            .dbname
            .map(|dbname| format!("/{dbname}"))
            .unwrap_or_default();
        let options = self
            .options
            .map(|options| format!("?{options}"))
            .unwrap_or_default();
        format!(
            "postgresql://{}:{}@{}:{}{}{}",
            self.user, self.password, self.host, self.port, dbname, options
        )
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct BlockscoutSettings {
    pub url: Url,
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
    #[serde(default = "default_blockscout_timeout")]
    pub timeout: u64,
}

fn default_blockscout_url() -> Url {
    "http://localhost:4000".parse().unwrap()
}

fn default_max_concurrent_requests() -> usize {
    5
}

fn default_blockscout_timeout() -> u64 {
    30
}

impl Default for BlockscoutSettings {
    fn default() -> Self {
        Self {
            url: default_blockscout_url(),
            max_concurrent_requests: default_max_concurrent_requests(),
            timeout: default_blockscout_timeout(),
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
            subgraphs_reader: Default::default(),
            database: DatabaseSettings {
                connect: DatabaseConnectSettings::Url(database_url),
            },
        }
    }
}
