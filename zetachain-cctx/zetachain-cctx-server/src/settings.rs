use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use serde::{Deserialize, Serialize};
use zetachain_cctx_logic::{client::RpcSettings, settings::IndexerSettings};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
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
    pub indexer: IndexerSettings,
    #[serde(default)]
    pub rpc: RpcSettings,
    #[serde(default = "default_websocket_settings")]
    pub websocket: WebSocketSettings,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebSocketSettings {
    #[serde(default = "default_websocket_enabled")]
    pub enabled: bool,
}

impl Default for WebSocketSettings {
    fn default() -> Self {
        Self {
            enabled: default_websocket_enabled(),
        }
    }
}

fn default_websocket_enabled() -> bool {
    true
}

fn default_websocket_settings() -> WebSocketSettings {
    WebSocketSettings::default()
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "ZETACHAIN_CCTX";
}

impl Settings {
    pub fn default(
        
        database_url: String,
        
    ) -> Self {
        Self {
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
            indexer: Default::default(),
            rpc: Default::default(),
            websocket: Default::default(),
        }
    }
}
