use blockscout_service_launcher::{
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use serde::Deserialize;
use std::{path::PathBuf, str::FromStr};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub metrics: MetricsSettings,
    #[serde(default)]
    pub tracing: TracingSettings,
    #[serde(default)]
    pub jaeger: JaegerSettings,

    pub chains_config: Option<PathBuf>,

    #[serde(default)]
    pub eth_bytecode_db: EthBytecodeDbSettings,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "PROXY_VERIFIER";
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EthBytecodeDbSettings {
    #[serde(default = "default_eth_bytecode_db_http_url")]
    pub http_url: url::Url,
    #[serde(default = "default_eth_bytecode_db_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub probe_url: bool,
    pub api_key: Option<String>,
}

impl Default for EthBytecodeDbSettings {
    fn default() -> Self {
        Self {
            http_url: default_eth_bytecode_db_http_url(),
            max_retries: default_eth_bytecode_db_max_retries(),
            probe_url: Default::default(),
            api_key: Default::default(),
        }
    }
}

fn default_eth_bytecode_db_max_retries() -> u32 {
    3
}

fn default_eth_bytecode_db_http_url() -> url::Url {
    url::Url::from_str("https://eth-bytecode-db.services.blockscout.com").unwrap()
}
