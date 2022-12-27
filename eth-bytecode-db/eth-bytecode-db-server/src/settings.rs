use blockscout_service_launcher::{JaegerSettings, MetricsSettings, ServerSettings};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    pub server: ServerSettings,
    pub metrics: MetricsSettings,
    pub jaeger: JaegerSettings,
}

impl Settings {
    pub fn new() -> anyhow::Result<Self> {
        todo!()
    }
}
