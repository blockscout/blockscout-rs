use blockscout_service_launcher::{
    GrpcServerSettings, HttpServerSettings, JaegerSettings, MetricsSettings, ServerSettings,
};
use config::{Config, File};
use cron::Schedule;
use serde::{de, Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
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

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub db_url: String,
    pub run_migrations: bool,
    pub blockscout_db_url: String,
    #[serde_as(as = "DisplayFromStr")]
    pub update_schedule: Schedule,

    pub server: ServerSettings,
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
                    enabled: false,
                    addr: SocketAddr::from_str("0.0.0.0:8051").unwrap(),
                },
            },
            db_url: Default::default(),
            update_schedule: Schedule::from_str("0 0 1 * * * *").unwrap(),
            blockscout_db_url: Default::default(),
            run_migrations: Default::default(),
            metrics: Default::default(),
            jaeger: Default::default(),
            config_path: Default::default(),
        }
    }
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
