use std::time;

use crate::{
    celestia::settings::IndexerSettings as CelestiaSettings,
    eigenda::settings::IndexerSettings as EigendaSettings,
};
use serde::Deserialize;
use serde_with::serde_as;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum DASettings {
    Celestia(CelestiaSettings),
    EigenDA(EigendaSettings),
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    pub da: DASettings,
    pub concurrency: u32,
    #[serde(default = "default_restart_delay")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub restart_delay: time::Duration,
    #[serde(default = "default_polling_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub polling_interval: time::Duration,
    #[serde(default = "default_retry_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub retry_interval: time::Duration,
    #[serde(default = "default_catchup_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub catchup_interval: time::Duration,
}

fn default_polling_interval() -> time::Duration {
    time::Duration::from_secs(12)
}

fn default_retry_interval() -> time::Duration {
    time::Duration::from_secs(180)
}

fn default_restart_delay() -> time::Duration {
    time::Duration::from_secs(60)
}

fn default_catchup_interval() -> time::Duration {
    time::Duration::from_secs(0)
}

impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            da: DASettings::Celestia(CelestiaSettings::default()),
            concurrency: 2,
            restart_delay: default_restart_delay(),
            polling_interval: default_polling_interval(),
            retry_interval: default_retry_interval(),
            catchup_interval: default_catchup_interval(),
        }
    }
}
