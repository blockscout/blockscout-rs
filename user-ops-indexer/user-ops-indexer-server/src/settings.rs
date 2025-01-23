use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use serde::Deserialize;
use user_ops_indexer_logic::indexer::settings::IndexerSettings;

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

impl Settings {
    pub fn default(database_url: String) -> Self {
        Self {
            server: Default::default(),
            metrics: Default::default(),
            tracing: Default::default(),
            jaeger: Default::default(),
            database: DatabaseSettings {
                connect: DatabaseConnectSettings::Url(database_url),
                connect_options: Default::default(),
                create_database: false,
                run_migrations: false,
            },
            api: ApiSettings { max_page_size: 100 },
            indexer: Default::default(),
        }
    }
}
