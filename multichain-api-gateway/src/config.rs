use crate::cli;
use clap::Parser;

use crate::cli::Args;
use serde::Deserialize;
use std::{net::SocketAddr, str::FromStr};
use url::Url;

/// An instance of the maintained networks in Blockscout.
/// Semantic: (network, chain)
/// e.g."blockscout.com/eth/mainnet" -> ("eth", "mainnet")
#[derive(Deserialize, Clone, Debug, PartialEq)]
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
}

impl Default for BlockscoutSettings {
    fn default() -> Self {
        Self {
            base_url: Url::parse("https://blockscout.com/").unwrap(),
            instances: vec![],
            concurrent_requests: 10,
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
