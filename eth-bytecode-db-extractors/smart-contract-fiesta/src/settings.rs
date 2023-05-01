use config::{Config, File};
use serde::{de, Deserialize};
use std::path::PathBuf;

/// Wrapper under [`serde::de::IgnoredAny`] which implements
/// [`PartialEq`] and [`Eq`] for fields to be ignored.
#[derive(Copy, Clone, Debug, Default, Deserialize)]
struct IgnoredAny(de::IgnoredAny);

impl PartialEq for IgnoredAny {
    fn eq(&self, _other: &Self) -> bool {
        // We ignore that values, so they should not impact the equality
        true
    }
}

impl Eq for IgnoredAny {}

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

    pub eth_bytecode_db_url: String,

    #[serde(default = "default_n_threads")]
    pub n_threads: usize,

    // Is required as we deny unknown fields, but allow users provide
    // path to config through PREFIX__CONFIG env variable. If removed,
    // the setup would fail with `unknown field `config`, expected one of...`
    #[serde(default, rename = "config")]
    config_path: IgnoredAny,
}

fn default_blockscout_url() -> String {
    "https://blockscout.com/eth/mainnet".to_string()
}

fn default_etherscan_url() -> String {
    "https://api.etherscan.io".to_string()
}

fn default_n_threads() -> usize {
    4
}

impl Settings {
    pub fn new() -> anyhow::Result<Self> {
        let config_path = std::env::var("FIESTA_EXTRACTOR__CONFIG");

        let mut builder = Config::builder();
        if let Ok(config_path) = config_path {
            builder = builder.add_source(File::with_name(&config_path));
        };
        // Use `__` so that it would be possible to address keys with underscores in names (e.g. `access_key`)
        builder = builder
            .add_source(config::Environment::with_prefix("FIESTA_EXTRACTOR").separator("__"));

        let settings: Settings = builder.build()?.try_deserialize()?;

        settings.validate()?;

        Ok(settings)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.import_dataset && self.dataset.is_none() {
            return Err(anyhow::anyhow!(
                "`dataset` path should be specified if `import_dataset` is enabled"
            ));
        }

        Ok(())
    }
}
