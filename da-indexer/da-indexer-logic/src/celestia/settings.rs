use std::time;

use serde::Deserialize;
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    pub rpc: RpcSettings,
    pub concurrency: u32,
    pub start_height: Option<u64>,
    #[serde(default = "default_restart_delay")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub restart_delay: time::Duration,
    #[serde(default = "default_polling_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub polling_interval: time::Duration,
    #[serde(default = "default_retry_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub retry_interval: time::Duration,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RpcSettings {
    pub url: String,
    pub auth_token: Option<String>,
    #[serde(default = "default_max_request_size")]
    pub max_request_size: u32,
    #[serde(default = "default_max_response_size")]
    pub max_response_size: u32,
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

fn default_max_request_size() -> u32 {
    100 * 1024 * 1024 // 100 Mb
}

fn default_max_response_size() -> u32 {
    100 * 1024 * 1024 // 100 Mb
}

impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            concurrency: 1,
            start_height: None,
            restart_delay: default_restart_delay(),
            polling_interval: default_polling_interval(),
            retry_interval: default_retry_interval(),
            rpc: RpcSettings {
                url: "http://localhost:26658".to_string(),
                auth_token: None,
                max_request_size: default_max_request_size(),
                max_response_size: default_max_response_size(),
            },
        }
    }
}
