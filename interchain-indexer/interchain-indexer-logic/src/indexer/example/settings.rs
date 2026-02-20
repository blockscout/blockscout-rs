use serde::Deserialize;
use serde_with::serde_as;
use std::time::Duration;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExampleIndexerSettings {
    #[serde(default = "default_fetch_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
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
