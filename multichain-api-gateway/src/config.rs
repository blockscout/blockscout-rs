use std::{net::SocketAddr, str::FromStr};

use clap::Parser;
use serde::Deserialize;
use url::Url;

use crate::{cli, cli::Args};

/// An instance of the maintained networks in Blockscout.
/// Semantic: (network, chain)
/// e.g."blockscout.com/eth/mainnet" -> ("eth", "mainnet")
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Instance(pub String, pub String);

/// Settings for the Blockscout API
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct BlockscoutSettings {
    /// The base URL of the Blockscout API.
    pub base_url: Url,

    pub instances: Vec<Instance>,

    /// The number of concurrent requests to be made to the Blockscout API from a server's thread worker.
    pub concurrent_requests: usize,

    /// The timeout of waiting for response from the Blockscout API.
    pub request_timeout: std::time::Duration,
}

impl Default for BlockscoutSettings {
    fn default() -> Self {
        Self {
            base_url: Url::parse("https://blockscout.com/").unwrap(),
            instances: vec![],
            concurrent_requests: 10,
            request_timeout: std::time::Duration::from_secs(60),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ServerSettings {
    pub addr: SocketAddr,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            addr: SocketAddr::from_str("0.0.0.0:8044").unwrap(),
        }
    }
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct Settings {
    pub server: ServerSettings,
    pub blockscout: BlockscoutSettings,
}

pub fn get_config() -> Settings {
    let args: Args = cli::Args::parse();

    let mut config: Settings = match args.config_path {
        Some(path) => match std::fs::read_to_string(path.clone()) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => {
                    println!("Custom config loaded from {}", path.display());
                    config
                }
                Err(e) => {
                    eprintln!("Failed to parse config: {}", e);
                    println!("Using default config");
                    Settings::default()
                }
            },
            Err(e) => {
                eprintln!("Failed to read config file: {}", e);
                println!("Using default config");
                Settings::default()
            }
        },
        None => {
            println!("Custom config wasn't provided.");
            println!("Using default config");
            Settings::default()
        }
    };

    if let Some(base_url) = args.base_url {
        config.blockscout.base_url = base_url;
    }
    if let Some(concurrent_requests) = args.concurrent_requests {
        config.blockscout.concurrent_requests = concurrent_requests;
    }
    if let Some(address) = args.address {
        config.server.addr = address;
    }

    config
}
