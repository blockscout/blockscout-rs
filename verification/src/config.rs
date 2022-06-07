use clap::Parser;
use config::{Config as LibConfig, File};
use serde::Deserialize;
use std::{net::SocketAddr, str::FromStr};

use crate::cli;

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfiguration,
    pub sourcify: SourcifyConfiguration,
}

#[derive(Deserialize, Clone)]
pub struct ServerConfiguration {
    pub addr: SocketAddr,
}

impl Default for ServerConfiguration {
    fn default() -> Self {
        Self {
            addr: SocketAddr::from_str("0.0.0.0:8043").unwrap(),
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct SourcifyConfiguration {
    pub api_url: String,
}

impl Default for SourcifyConfiguration {
    fn default() -> Self {
        Self {
            api_url: "https://sourcify.dev/server/".to_string(),
        }
    }
}

impl Config {
    pub fn parse() -> Result<Self, config::ConfigError> {
        let args = cli::Args::parse();
        let mut builder =
            LibConfig::builder().add_source(config::Environment::with_prefix("VERIFICATION"));
        if args.config_path.exists() {
            builder = builder.add_source(File::from(args.config_path));
        }
        builder
            .build()
            .expect("Failed to build config")
            .try_deserialize()
    }
}
