use serde::Deserialize;
use serde_with::serde_as;
use std::time::Duration;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChainInfoServiceSettings {
    // If the chain name is unknown,
    // do not retry DB query during this interval.
    #[serde(default = "default_cooldown_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub cooldown_interval: Duration,
}

fn default_cooldown_interval() -> Duration {
    Duration::from_secs(60)
}

impl Default for ChainInfoServiceSettings {
    fn default() -> Self {
        Self {
            cooldown_interval: default_cooldown_interval(),
        }
    }
}
