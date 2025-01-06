use blockscout_service_launcher::{
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use serde::Deserialize;
use url::Url;

#[derive(Debug, Default, Deserialize, Clone, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub server: ServerSettings,
    pub metrics: MetricsSettings,
    pub tracing: TracingSettings,
    pub jaeger: JaegerSettings,
    pub docker_api: DockerApiSettings,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct DockerApiSettings {
    pub addr: Url,
}

impl Default for DockerApiSettings {
    fn default() -> Self {
        Self {
            addr: Url::parse("unix:///var/run/docker.sock").expect("default docker api url"),
        }
    }
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "STYLUS_VERIFIER";
}
