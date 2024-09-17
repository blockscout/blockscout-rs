use blockscout_service_launcher::{
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub server: ServerSettings,
    pub metrics: MetricsSettings,
    pub tracing: TracingSettings,
    pub jaeger: JaegerSettings,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "STYLUS_VERIFIER";
}
