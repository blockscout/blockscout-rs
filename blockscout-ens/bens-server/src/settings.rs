use std::collections::HashMap;

use blockscout_service_launcher::{
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use serde::Deserialize;
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

    pub database: DatabaseSettings,
    #[serde(default)]
    pub blockscout: BlockscoutSettings,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "BENS";
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
pub struct NetworkConfig {
    pub url: Url,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct BlockscoutSettings {
    #[serde(default = "default_networks")]
    pub networks: HashMap<i64, NetworkConfig>,
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
    #[serde(default = "default_blockscout_timeout")]
    pub timeout: u64,
}

fn default_networks() -> HashMap<i64, NetworkConfig> {
    HashMap::from_iter([
        (
            1,
            NetworkConfig {
                url: "https://eth.blockscout.com".parse().unwrap(),
            },
        ),
        (
            30,
            NetworkConfig {
                url: "https://rootstock.blockscout.com".parse().unwrap(),
            },
        ),
    ])
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
            networks: default_networks(),
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
            database: DatabaseSettings {
                connect: DatabaseConnectSettings::Url(database_url),
            },
            blockscout: Default::default(),
        }
    }
}
