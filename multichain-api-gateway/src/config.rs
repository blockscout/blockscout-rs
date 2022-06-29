use crate::cli;
use clap::Parser;

use crate::cli::Args;
use serde::Deserialize;
use std::net::SocketAddr;
use std::str::FromStr;
use url::Url;

/// An instance of the maintained networks in Blockscout.
/// Semantic: (network, chain)
/// e.g."blockscout.com/eth/mainnet" -> ("eth", "mainnet")
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Instance(pub String, pub String);

/// Settings for the Blockscout API
#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct BlockScoutSettings {
    /// The base URL of the Blockscout API.
    #[serde(default = "default_base_url")]
    pub base_url: Url,

    pub instances: Vec<Instance>,

    /// The number of concurrent requests to be made to the Blockscout API from a server's thread worker.
    #[serde(default = "default_concurrent_requests")]
    pub concurrent_requests: usize,
}

impl Default for BlockScoutSettings {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            instances: vec![],
            concurrent_requests: default_concurrent_requests(),
        }
    }
}

fn default_base_url() -> Url {
    Url::parse("https://blockscout.com/").unwrap()
}

fn default_concurrent_requests() -> usize {
    10
}

#[derive(Deserialize, Clone)]
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

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub server: ServerSettings,
    pub block_scout: BlockScoutSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerSettings::default(),
            block_scout: BlockScoutSettings::default(),
        }
    }
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

    args.base_url.map(|url| config.block_scout.base_url = url);
    args.concurrent_requests
        .map(|concurrent_requests| config.block_scout.concurrent_requests = concurrent_requests);
    args.address.map(|address| config.server.addr = address);

    config
}
