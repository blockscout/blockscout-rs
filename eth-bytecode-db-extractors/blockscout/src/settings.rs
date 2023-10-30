use blockscout_service_launcher::launcher;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub database_url: String,
    #[serde(default)]
    pub create_database: bool,
    #[serde(default)]
    pub run_migrations: bool,

    #[serde(default)]
    pub force_import: bool,

    #[serde(default = "default_chain_id")]
    pub chain_id: u64,

    #[serde(default = "default_blockscout_url")]
    pub blockscout_url: String,
    #[serde(default = "default_limit_requests_per_second")]
    pub limit_requests_per_second: u32,

    pub eth_bytecode_db_url: String,
    pub eth_bytecode_db_api_key: String,

    #[serde(default = "default_n_threads")]
    pub n_threads: usize,
}

impl launcher::ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "BLOCKSCOUT_EXTRACTOR";
}

// fn default_chain_id() -> u64 {
//     1
// }

// fn default_blockscout_url() -> String {
//     "https://eth.blockscout.com".to_string()
// }

fn default_chain_id() -> u64 {
    5
}

fn default_blockscout_url() -> String {
    "https://eth-goerli.blockscout.com".to_string()
}

fn default_limit_requests_per_second() -> u32 {
    u32::MAX
}

fn default_n_threads() -> usize {
    1
}

// fn default_search_n_threads() -> usize {
//     1
// }
