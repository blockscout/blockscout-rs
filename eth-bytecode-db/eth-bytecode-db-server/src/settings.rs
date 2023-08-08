use blockscout_service_launcher::{
    JaegerSettings, MetricsSettings, ServerSettings, TracingSettings,
};
use config::{Config, File};
use serde::{de, Deserialize};
use serde_with::{serde_as, DisplayFromStr};
use std::time::Duration;

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
    pub create_database: bool,
    pub run_migrations: bool,
    #[serde(default)]
    pub sourcify: SourcifySettings,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerifierSettings {
    #[serde_as(as = "DisplayFromStr")]
    pub uri: tonic::transport::Uri,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourcifySettings {
    pub enabled: bool,
    pub base_url: String,
    /// The maximum duration for which attempts to send a request will be processed.
    pub total_request_duration: Duration,
}

impl Default for SourcifySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "https://sourcify.dev/server/".to_string(),
            total_request_duration: Duration::from_secs(60),
        }
    }
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
                sourcify: Default::default(),
            },
            verifier: VerifierSettings { uri: verifier_uri },
            config_path: Default::default(),
        }
    }
}
