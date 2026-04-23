use blockscout_service_launcher::launcher::ConfigSettings;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub database_url: String,
    #[serde(default)]
    pub create_database: bool,
    #[serde(default)]
    pub run_migrations: bool,

    #[serde(default)]
    pub dataset: Option<PathBuf>,
    #[serde(default)]
    pub import_dataset: bool,

    #[serde(default = "default_blockscout_url")]
    pub blockscout_url: String,

    #[serde(default = "default_etherscan_url")]
    pub etherscan_url: String,
    pub etherscan_api_key: String,
    #[serde(default = "default_etherscan_limit_requests_per_second")]
    pub etherscan_limit_requests_per_second: u32,

    pub eth_bytecode_db_url: String,

    #[serde(default = "default_n_threads")]
    pub n_threads: usize,

    #[serde(default)]
    pub search_enabled: bool,
    #[serde(default = "default_search_n_threads")]
    pub search_n_threads: usize,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "FIESTA_EXTRACTOR__CONFIG";

    fn validate(&self) -> anyhow::Result<()> {
        if self.import_dataset && self.dataset.is_none() {
            return Err(anyhow::anyhow!(
                "`dataset` path should be specified if `import_dataset` is enabled"
            ));
        }

        Ok(())
    }
}

fn default_blockscout_url() -> String {
    "https://blockscout.com/eth/mainnet".to_string()
}

fn default_etherscan_url() -> String {
    "https://api.etherscan.io".to_string()
}

/// Default value for the basic (free) plan
fn default_etherscan_limit_requests_per_second() -> u32 {
    5
}

fn default_n_threads() -> usize {
    4
}

fn default_search_n_threads() -> usize {
    1
}
