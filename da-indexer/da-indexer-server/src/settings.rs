use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use da_indexer_logic::{
    celestia::l2_router::settings::L2RouterSettings, settings::IndexerSettings,
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

    pub database: Option<DatabaseSettings>,
    pub indexer: Option<IndexerSettings>,
    pub l2_router: Option<L2RouterSettings>,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "DA_INDEXER";
}

impl Settings {
    pub fn default(database_url: String) -> Self {
        Self {
            server: Default::default(),
            metrics: Default::default(),
            tracing: Default::default(),
            jaeger: Default::default(),
            database: Some(DatabaseSettings {
                connect: DatabaseConnectSettings::Url(database_url),
                create_database: Default::default(),
                run_migrations: Default::default(),
            }),
            indexer: Some(Default::default()),
            l2_router: None,
        }
    }
}
