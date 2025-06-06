use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use serde::Deserialize;
use std::path::PathBuf;
use tac_operation_lifecycle_logic::{client::settings::RpcSettings, settings::IndexerSettings};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
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
    #[serde(default = "default_swagger_path")]
    pub swagger_path: PathBuf,

    pub database: DatabaseSettings,

    pub indexer: Option<IndexerSettings>,
    pub rpc: RpcSettings,
}

fn default_swagger_path() -> PathBuf {
    blockscout_endpoint_swagger::default_swagger_path_from_service_name("tac-operation-lifecycle")
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "TAC_OPERATION_LIFECYCLE";
}

impl Settings {
    pub fn default(database_url: String) -> Self {
        Self {
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
            indexer: None,
            rpc: RpcSettings::default(),
        }
    }
}
