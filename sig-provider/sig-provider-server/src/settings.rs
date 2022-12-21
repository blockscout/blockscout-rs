use config::{Config, File};
use serde::{de, Deserialize, Serialize};
use startuper::{
    GrpcServerSettings, HttpServerSettings, JaegerSettings, MetricsSettings, ServerSettings,
};
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub server: ServerSettings,
    pub sources: SourcesSettings,
    pub metrics: MetricsSettings,
    pub jaeger: JaegerSettings,

    // Is required as we deny unknown fields, but allow users provide
    // path to config through PREFIX__CONFIG env variable. If removed,
    // the setup would fail with `unknown field `config`, expected one of...`
    #[serde(skip_serializing, rename = "config")]
    config_path: IgnoredAny,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerSettings {
                http: HttpServerSettings {
                    enabled: true,
                    addr: SocketAddr::from_str("0.0.0.0:8050").unwrap(),
                },
                grpc: GrpcServerSettings {
                    enabled: true,
                    addr: SocketAddr::from_str("0.0.0.0:8051").unwrap(),
                },
            },
            sources: Default::default(),
            metrics: Default::default(),
            jaeger: Default::default(),
            config_path: Default::default(),
        }
    }
}

impl Settings {
    pub fn new() -> anyhow::Result<Self> {
        let config_path = std::env::var("SIG_PROVIDER__CONFIG");

        let mut builder = Config::builder();
        if let Ok(config_path) = config_path {
            builder = builder.add_source(File::with_name(&config_path));
        };
        // Use `__` so that it would be possible to address keys with underscores in names (e.g. `access_key`)
        builder =
            builder.add_source(config::Environment::with_prefix("SIG_PROVIDER").separator("__"));

        let settings: Settings = builder.build()?.try_deserialize()?;

        Ok(settings)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SourcesSettings {
    pub fourbyte: url::Url,
    pub sigeth: url::Url,
}

impl Default for SourcesSettings {
    fn default() -> Self {
        Self {
            fourbyte: url::Url::parse("https://www.4byte.directory/").unwrap(),
            sigeth: url::Url::parse("https://sig.eth.samczsun.com/").unwrap(),
        }
    }
}
