use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExampleIndexerSettings {
    #[serde(default = "default_fetch_interval")]
    pub fetch_interval: Duration,
}

fn default_fetch_interval() -> Duration {
    Duration::from_secs(1)
}

impl Default for ExampleIndexerSettings {
    fn default() -> Self {
        Self {
            fetch_interval: default_fetch_interval(),
        }
    }
}
