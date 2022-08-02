use crate::consts::DEFAULT_COMPILER_LIST;
use anyhow::anyhow;
use config::{Config, File};
use cron::Schedule;
use serde::{de::IgnoredAny, Deserialize};
use std::{net::SocketAddr, num::NonZeroUsize, str::FromStr};
use url::Url;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub server: ServerSettings,
    pub solidity: SoliditySettings,
    pub sourcify: SourcifySettings,
    pub metrics: MetricsSettings,

    pub config: IgnoredAny,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSettings {
    pub addr: SocketAddr,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            addr: SocketAddr::from_str("0.0.0.0:8043").expect("should be valid url"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SoliditySettings {
    pub enabled: bool,
    pub compilers_list_url: Url,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub refresh_versions_schedule: Schedule,
}

impl Default for SoliditySettings {
    fn default() -> Self {
        Self {
            compilers_list_url: Url::try_from(DEFAULT_COMPILER_LIST).expect("valid url"),
            enabled: true,
            refresh_versions_schedule: Schedule::from_str("0 0 * * * * *").unwrap(), // every hour
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SourcifySettings {
    pub enabled: bool,
    pub api_url: Url,
    /// Number of attempts the server makes to Sourcify API.
    /// Should be at least one. Set to `3` by default.
    pub verification_attempts: NonZeroUsize,
    pub request_timeout: u64,
}

impl Default for SourcifySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            api_url: Url::try_from("https://sourcify.dev/server/").expect("valid url"),
            verification_attempts: NonZeroUsize::new(3).expect("Is not zero"),
            request_timeout: 10,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsSettings {
    pub endpoint: String,
    pub addr: SocketAddr,
}

impl Default for MetricsSettings {
    fn default() -> Self {
        Self {
            endpoint: "/metrics".to_string(),
            addr: SocketAddr::from_str("0.0.0.0:6060").expect("should be valid url"),
        }
    }
}

impl Settings {
    pub fn new() -> anyhow::Result<Self> {
        let config_path = std::env::var("VERIFICATION_CONFIG");

        let mut builder = Config::builder();
        if let Ok(config_path) = config_path {
            builder = builder.add_source(File::with_name(&config_path));
        };
        builder = builder.add_source(config::Environment::with_prefix("VERIFICATION"));

        builder
            .build()?
            .try_deserialize()
            .map_err(|err| anyhow!(err))
    }
}
