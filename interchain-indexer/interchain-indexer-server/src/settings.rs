use std::path::PathBuf;

use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use interchain_indexer_logic::{
    ChainInfoServiceSettings, TokenInfoServiceSettings,
    avalanche::settings::AvalancheIndexerSettings, example::settings::ExampleIndexerSettings,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub chains_config: PathBuf,
    pub bridges_config: PathBuf,

    #[serde(default)]
    pub token_info: TokenInfoServiceSettings,

    #[serde(default)]
    pub chain_info: ChainInfoServiceSettings,

    #[serde(default)]
    pub example_indexer: ExampleIndexerSettings,

    #[serde(default)]
    pub avalanche_indexer: AvalancheIndexerSettings,

    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub metrics: MetricsSettings,
    #[serde(default)]
    pub tracing: TracingSettings,
    #[serde(default)]
    pub jaeger: JaegerSettings,
    #[serde(default = "default_swagger_path")]
    pub swagger_path: PathBuf,

    pub database: DatabaseSettings,

    #[serde(default)]
    pub api: ApiSettings,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "INTERCHAIN_INDEXER";
}

fn default_swagger_path() -> PathBuf {
    blockscout_endpoint_swagger::default_swagger_path_from_service_name("interchain-indexer")
}

impl Settings {
    pub fn default(database_url: String) -> Self {
        Self {
            chains_config: PathBuf::from("config/omnibridge/chains.json"),
            bridges_config: PathBuf::from("config/omnibridge/bridges.json"),
            token_info: TokenInfoServiceSettings::default(),
            chain_info: ChainInfoServiceSettings::default(),
            example_indexer: Default::default(),
            avalanche_indexer: Default::default(),
            server: Default::default(),
            metrics: Default::default(),
            tracing: Default::default(),
            jaeger: Default::default(),
            swagger_path: default_swagger_path(),
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
    pub use_pagination_token: bool,
}

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            default_page_size: 50,
            max_page_size: 100,
            use_pagination_token: true,
        }
    }
}
