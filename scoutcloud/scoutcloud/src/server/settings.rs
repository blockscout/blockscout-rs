use blockscout_service_launcher::{
    database::DatabaseSettings,
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
    pub github: GithubSettings,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "SCOUTCLOUD";
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GithubSettings {
    pub token: String,
    pub owner: String,
    pub repo: String,
    #[serde(default)]
    pub branch: Option<String>,
}
