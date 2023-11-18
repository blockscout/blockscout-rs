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

    pub eth_bytecode_db_database: String,
    pub eth_bytecode_db_url: String,

    #[serde(default = "default_n_threads")]
    pub n_threads: usize,
}

impl launcher::ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "BLOCKSCOUT_EXTRACTOR";
}

fn default_n_threads() -> usize {
    1
}

// fn default_search_n_threads() -> usize {
//     1
// }
