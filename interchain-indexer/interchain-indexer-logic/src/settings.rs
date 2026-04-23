use serde::Deserialize;
use serde_with::serde_as;
use std::time::Duration;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MessageBufferSettings {
    #[serde(default = "default_hot_ttl")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub hot_ttl: Duration,
    #[serde(default = "default_maintenance_interval")]
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub maintenance_interval: Duration,
}

fn default_hot_ttl() -> Duration {
    Duration::from_secs(10)
}

fn default_maintenance_interval() -> Duration {
    Duration::from_millis(500)
}

impl Default for MessageBufferSettings {
    fn default() -> Self {
        Self {
            hot_ttl: default_hot_ttl(),
            maintenance_interval: default_maintenance_interval(),
        }
    }
}
