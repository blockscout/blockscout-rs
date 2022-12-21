use config::{Config, File};
use serde::{de, Deserialize, Serialize};
use std::{net::SocketAddr, str::FromStr};

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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub stats: StatsSettings,
    pub server: ServerSettings,

    // Is required as we deny unknown fields, but allow users provide
    // path to config through PREFIX__CONFIG env variable. If removed,
    // the setup would fail with `unknown field `config`, expected one of...`
    #[serde(skip_serializing, rename = "config")]
    config_path: IgnoredAny,
}

impl Settings {
    pub fn new() -> anyhow::Result<Self> {
        let config_path = std::env::var("STATS__CONFIG");

        let mut builder = Config::builder();
        if let Ok(config_path) = config_path {
            builder = builder.add_source(File::with_name(&config_path));
        };
        // Use `__` so that it would be possible to address keys with underscores in names (e.g. `access_key`)
        builder = builder.add_source(config::Environment::with_prefix("STATS").separator("__"));

        let settings: Settings = builder.build()?.try_deserialize()?;

        Ok(settings)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct StatsSettings {
    pub db_url: String,
    // pub blockscout_db_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSettings {
    pub http: HttpServerSettings,
    pub grpc: GrpcServerSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpServerSettings {
    pub enabled: bool,
    pub addr: SocketAddr,
}

impl Default for HttpServerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            addr: SocketAddr::from_str("0.0.0.0:8050").expect("valid addr"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GrpcServerSettings {
    pub enabled: bool,
    pub addr: SocketAddr,
}

impl Default for GrpcServerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            addr: SocketAddr::from_str("0.0.0.0:8051").expect("valid addr"),
        }
    }
}
