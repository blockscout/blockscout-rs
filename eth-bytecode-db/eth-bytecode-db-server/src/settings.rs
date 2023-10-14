use blockscout_service_launcher::{
    launcher::{MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use config::{Config, File};
use serde::{de, Deserialize};
use serde_with::{serde_as, DisplayFromStr};
use std::collections::HashMap;

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
    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub metrics: MetricsSettings,
    #[serde(default)]
    pub tracing: TracingSettings,
    #[serde(default)]
    pub jaeger: JaegerSettings,

    pub database: DatabaseSettings,
    pub verifier: VerifierSettings,
    #[serde(default)]
    pub sourcify: SourcifySettings,
    #[serde(default)]
    pub verifier_alliance_database: VerifierAllianceDatabaseSettings,

    #[serde(default)]
    pub authorized_keys: HashMap<String, ApiKey>,

    // Is required as we deny unknown fields, but allow users provide
    // path to config through PREFIX__CONFIG env variable. If removed,
    // the setup would fail with `unknown field `config`, expected one of...`
    #[serde(default, rename = "config")]
    config_path: IgnoredAny,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseSettings {
    pub url: String,
    #[serde(default)]
    pub create_database: bool,
    #[serde(default)]
    pub run_migrations: bool,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerifierSettings {
    #[serde_as(as = "DisplayFromStr")]
    pub uri: tonic::transport::Uri,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SourcifySettings {
    pub base_url: String,
    /// The maximum number of attempts to repeat requests in case of server side errors.
    pub max_retries: u32,
}

impl Default for SourcifySettings {
    fn default() -> Self {
        Self {
            base_url: "https://sourcify.dev/server/".to_string(),
            max_retries: 3,
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct VerifierAllianceDatabaseSettings {
    pub enabled: bool,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApiKey {
    pub key: String,
}

impl Settings {
    pub fn new() -> anyhow::Result<Self> {
        let config_path = std::env::var("ETH_BYTECODE_DB__CONFIG");

        let mut builder = Config::builder();
        if let Ok(config_path) = config_path {
            builder = builder.add_source(File::with_name(&config_path));
        };
        // Use `__` so that it would be possible to address keys with underscores in names (e.g. `first_key`)
        builder =
            builder.add_source(config::Environment::with_prefix("ETH_BYTECODE_DB").separator("__"));

        let settings: Settings = builder.build()?.try_deserialize()?;

        Ok(settings)
    }

    pub fn default(database_url: String, verifier_uri: tonic::transport::Uri) -> Self {
        Self {
            server: Default::default(),
            metrics: Default::default(),
            tracing: Default::default(),
            jaeger: Default::default(),
            database: DatabaseSettings {
                url: database_url,
                create_database: false,
                run_migrations: false,
            },
            verifier: VerifierSettings { uri: verifier_uri },
            sourcify: Default::default(),
            verifier_alliance_database: Default::default(),
            authorized_keys: Default::default(),
            config_path: Default::default(),
        }
    }
}
