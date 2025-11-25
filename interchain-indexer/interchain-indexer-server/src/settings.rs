use std::path::PathBuf;

use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use interchain_indexer_logic::example::settings::ExampleIndexerSettings;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub chains_config: PathBuf,
    pub bridges_config: PathBuf,

    #[serde(default)]
    pub example_indexer: ExampleIndexerSettings,

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
    pub api: ApiSettings,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "INTERCHAIN_INDEXER";
}

impl Settings {
    pub fn default(database_url: String) -> Self {
        Self {
            chains_config: PathBuf::from("config/omnibridge/chains.json"),
            bridges_config: PathBuf::from("config/omnibridge/bridges.json"),
            example_indexer: Default::default(),
            server: Default::default(),
            metrics: Default::default(),
            tracing: Default::default(),
            jaeger: Default::default(),
            database: DatabaseSettings {
                connect: DatabaseConnectSettings::Url(database_url),
                create_database: Default::default(),
                run_migrations: Default::default(),
                connect_options: Default::default(),
            },
            api: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ApiSettings {
    pub default_page_size: u32,
    pub max_page_size: u32,
}

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            default_page_size: 50,
            max_page_size: 100,
        }
    }
}
