use std::{net::SocketAddr, str::FromStr};

use config::{Config, File};
use serde::{de::IgnoredAny, Deserialize};
use std::time;
use url::Url;

/// An instance of the maintained networks in Blockscout.
/// Semantic: (network, chain)
/// e.g."blockscout.com/eth/mainnet" -> ("eth", "mainnet")
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Instance(pub String, pub String);

/// Settings for the Blockscout API
#[serde_with::serde_as]
#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct BlockscoutSettings {
    /// The base URL of the Blockscout API.
    pub base_url: Url,

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

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub server: ServerSettings,
    pub blockscout: BlockscoutSettings,

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
        builder = builder
            .add_source(config::Environment::with_prefix("MULTICHAIN_API_GATEWAY").separator("__"));

        println!("{:?}", builder);

        println!("{:?}", builder.build_cloned().unwrap());

        let settings: Settings = builder.build()?.try_deserialize()?;

        Ok(settings)
    }
}
