use std::{net::SocketAddr, str::FromStr};

use config::{Config, File};
use serde::{de::IgnoredAny, Deserialize};
use std::time;
use url::Url;

use serde_with::{As, DisplayFromStr};
use tracing::{event, Level};

/// An instance of the maintained networks in Blockscout.
/// Semantic: (network, chain)
/// e.g."blockscout.com/eth/mainnet" -> ("eth", "mainnet")
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Instance(pub String, pub String);

impl FromStr for Instance {
    type Err = String;
    /// "eth/mainnet" -> Instance("eth", "mainnet")
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('/');
        let network = parts.next().ok_or("missing network")?;
        let chain = parts.next().ok_or("missing chain")?;
        Ok(Instance(network.to_string(), chain.to_string()))
    }
}

/// Settings for the Blockscout API
#[serde_with::serde_as]
#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct BlockscoutSettings {
    /// The base URL of the Blockscout API.
    pub base_url: Url,

    #[serde(with = "As::<Vec<DisplayFromStr>>")]
    pub instances: Vec<Instance>,

    /// The number of concurrent requests to be made to the Blockscout API from a server's thread worker.
    pub concurrent_requests: usize,

    /// The timeout of waiting for response from the Blockscout API.
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub request_timeout: time::Duration,
}

impl Default for BlockscoutSettings {
    fn default() -> Self {
        Self {
            base_url: Url::parse("https://blockscout.com/").expect("should be correct base"),
            instances: vec![
                Instance("eth".into(), "mainnet".into()),
                Instance("xdai".into(), "mainnet".into()),
            ],
            concurrent_requests: 10,
            request_timeout: time::Duration::from_secs(60),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSettings {
    pub addr: SocketAddr,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            addr: SocketAddr::from_str("0.0.0.0:8044").expect("should be valid url"),
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

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub server: ServerSettings,
    pub blockscout: BlockscoutSettings,
    pub jaeger: JaegerSettings,

    // Is required as we deny unknown fields, but allow users provide
    // path to config through PREFIX__CONFIG env variable. If removed,
    // the setup would fail with `unknown field `config`, expected one of...`
    #[serde(rename = "config")]
    pub config_path: IgnoredAny,
}

impl Settings {
    pub fn new() -> anyhow::Result<Self> {
        let config_path = std::env::var("MULTICHAIN_API_GATEWAY__CONFIG");

        let mut builder = Config::builder();
        if let Ok(config_path) = config_path {
            builder = builder.add_source(File::with_name(&config_path));
        };
        let environment = config::Environment::with_prefix("MULTICHAIN_API_GATEWAY")
            .try_parsing(true)
            .separator("__")
            .list_separator(";")
            .with_list_parse_key("blockscout.instances");
        builder = builder.add_source(environment);

        let settings: Settings = builder.build()?.try_deserialize()?;

        event!(Level::INFO, settings = ?settings, "Initilized with settings");

        Ok(settings)
    }
}
