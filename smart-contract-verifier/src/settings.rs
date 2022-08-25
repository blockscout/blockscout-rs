use crate::consts::{DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_VYPER_COMPILER_LIST};
use anyhow::anyhow;
use config::{Config, File};
use cron::Schedule;
use serde::{de::IgnoredAny, Deserialize};
use std::{net::SocketAddr, num::NonZeroUsize, path::PathBuf, str::FromStr};
use url::Url;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub server: ServerSettings,
    pub solidity: SoliditySettings,
    pub vyper: VyperSettings,
    pub sourcify: SourcifySettings,
    pub metrics: MetricsSettings,
    pub jaeger: JaegerSettings,

    // Is required as we deny unknown fields, but allow users provide
    // path to config through PREFIX__CONFIG env variable. If removed,
    // the setup would fail with `unknown field `config`, expected one of...`
    #[serde(rename = "config")]
    pub config_path: IgnoredAny,
}

impl PartialEq for Settings {
    fn eq(&self, other: &Self) -> bool {
        self.server == other.server
            && self.solidity == other.solidity
            && self.sourcify == other.sourcify
            && self.metrics == other.metrics
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SoliditySettings {
    pub enabled: bool,
    pub compilers_dir: PathBuf,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub refresh_versions_schedule: Schedule,
    pub fetcher: FetcherSettings,
}

impl Default for SoliditySettings {
    fn default() -> Self {
        let mut default_dir = std::env::temp_dir();
        default_dir.push("solidity-compilers");
        Self {
            enabled: true,
            compilers_dir: default_dir,
            refresh_versions_schedule: Schedule::from_str("0 0 * * * * *").unwrap(), // every hour
            fetcher: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VyperSettings {
    pub enabled: bool,
    pub compilers_dir: PathBuf,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub refresh_versions_schedule: Schedule,
    pub list_url: Url,
}

impl Default for VyperSettings {
    fn default() -> Self {
        let mut default_dir = std::env::temp_dir();
        default_dir.push("vyper-compilers");
        Self {
            enabled: true,
            compilers_dir: default_dir,
            refresh_versions_schedule: Schedule::from_str("0 0 * * * * *").unwrap(), // every hour
            list_url: Url::try_from(DEFAULT_VYPER_COMPILER_LIST).expect("valid url"),
        }
    }
}

#[derive(Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum FetcherSettings {
    List(ListFetcherSettings),
    S3(S3FetcherSettings),
}

impl Default for FetcherSettings {
    fn default() -> Self {
        Self::List(Default::default())
    }
}

#[derive(Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct ListFetcherSettings {
    pub list_url: Url,
}

impl Default for ListFetcherSettings {
    fn default() -> Self {
        Self {
            list_url: Url::try_from(DEFAULT_SOLIDITY_COMPILER_LIST).expect("valid url"),
        }
    }
}

#[derive(Deserialize, Default, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct S3FetcherSettings {
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub bucket: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsSettings {
    pub enabled: bool,
    pub addr: SocketAddr,
    pub route: String,
}

impl Default for MetricsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            addr: SocketAddr::from_str("0.0.0.0:6060").expect("should be valid url"),
            route: "/metrics".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JaegerSettings {
    pub enabled: bool,
    pub agent_endpoint: String,
}

impl Default for JaegerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            agent_endpoint: "localhost:6831".to_string(),
        }
    }
}

impl Settings {
    pub fn new() -> anyhow::Result<Self> {
        let config_path = std::env::var("SMART_CONTRACT_VERIFIER__CONFIG");

        let mut builder = Config::builder();
        if let Ok(config_path) = config_path {
            builder = builder.add_source(File::with_name(&config_path));
        };
        // Use `__` so that it would be possible to address keys with underscores in names (e.g. `access_key`)
        builder = builder.add_source(
            config::Environment::with_prefix("SMART_CONTRACT_VERIFIER").separator("__"),
        );

        let settings: Settings = builder.build()?.try_deserialize()?;

        settings.validate()?;

        Ok(settings)
    }

    fn validate(&self) -> anyhow::Result<()> {
        // Validate s3 fetcher
        if let FetcherSettings::S3(settings) = &self.solidity.fetcher {
            if settings.region.is_none() && settings.endpoint.is_none() {
                return Err(anyhow!("for s3 fetcher settings at least one of `region` or `endpoint` should be defined"));
            }
        };

        Ok(())
    }
}
