use blockscout_service_launcher::{database::DatabaseSettings, launcher};
use serde::Deserialize;
use std::str::FromStr;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct  Settings {
    pub database: DatabaseSettings,
    #[serde(default = "default_chain_id")]
    pub chain_id: String,
    #[serde(default = "default_explorer_url")]
    pub explorer_url: url::Url,
    pub explorer_api_key: String,
    #[serde(default = "default_limit_requests_per_second")]
    pub limit_requests_per_second: u32,
    #[serde(default = "default_n_threads")]
    pub n_threads: usize,
}

impl launcher::ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "EXPLORER_EXTRACTOR";
}

fn default_chain_id() -> String {
    "1".to_string()
}

fn default_explorer_url() -> url::Url {
    url::Url::from_str("https://eth.blockscout.com/").unwrap()
}

fn default_limit_requests_per_second() -> u32 {
    15
}

fn default_n_threads() -> usize {
    1
}
