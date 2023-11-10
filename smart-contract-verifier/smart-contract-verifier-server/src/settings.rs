use anyhow::anyhow;
use blockscout_service_launcher::{
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use cron::Schedule;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use smart_contract_verifier::{
    DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_SOURCIFY_HOST, DEFAULT_VYPER_COMPILER_LIST,
};
use std::{
    num::{NonZeroU32, NonZeroUsize},
    path::PathBuf,
    str::FromStr,
};
use url::Url;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub server: ServerSettings,
    pub solidity: SoliditySettings,
    pub vyper: VyperSettings,
    pub sourcify: SourcifySettings,
    pub metrics: MetricsSettings,
    pub jaeger: JaegerSettings,
    pub tracing: TracingSettings,
    pub compilers: CompilersSettings,
    pub extensions: ExtensionsSettings,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SoliditySettings {
    pub enabled: bool,
    pub compilers_dir: PathBuf,
    #[serde_as(as = "DisplayFromStr")]
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

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VyperSettings {
    pub enabled: bool,
    pub compilers_dir: PathBuf,
    #[serde_as(as = "DisplayFromStr")]
    pub refresh_versions_schedule: Schedule,
    pub fetcher: FetcherSettings,
}

impl Default for VyperSettings {
    fn default() -> Self {
        let mut default_dir = std::env::temp_dir();
        default_dir.push("vyper-compilers");
        let fetcher = FetcherSettings::List(ListFetcherSettings {
            list_url: Url::try_from(DEFAULT_VYPER_COMPILER_LIST).expect("valid url"),
        });
        Self {
            enabled: true,
            compilers_dir: default_dir,
            refresh_versions_schedule: Schedule::from_str("0 0 * * * * *").unwrap(), // every hour
            fetcher,
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
    pub verification_attempts: NonZeroU32,
    pub request_timeout: u64,
}

impl Default for SourcifySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            api_url: Url::try_from(DEFAULT_SOURCIFY_HOST).expect("valid url"),
            verification_attempts: NonZeroU32::new(3).expect("Is not zero"),
            request_timeout: 15,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompilersSettings {
    pub max_threads: NonZeroUsize,
}

impl Default for CompilersSettings {
    fn default() -> Self {
        let max_threads = std::thread::available_parallelism().unwrap_or_else(|e| {
            tracing::warn!("cannot get number of CPU cores: {}", e);
            NonZeroUsize::new(8).unwrap()
        });
        Self { max_threads }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExtensionsSettings {
    pub solidity: Extensions,
    pub sourcify: Extensions,
    pub vyper: Extensions,
}

#[derive(Default, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Extensions {
    #[cfg(feature = "sig-provider-extension")]
    pub sig_provider: Option<sig_provider_extension::Config>,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "SMART_CONTRACT_VERIFIER";

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
