use crate::cli;
use clap::Parser;
use config::{Config as LibConfig, File};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfiguration,
    pub uml_creator: UMLCreatorConfiguration,
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct ServerConfiguration {
    pub addr: SocketAddr,
}

impl Default for ServerConfiguration {
    fn default() -> Self {
        Self {
            addr: ("0.0.0.0:8043").parse().expect("should be valid url"),
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct UMLCreatorConfiguration {
    pub tmp_dir: PathBuf,
}

impl Default for UMLCreatorConfiguration {
    fn default() -> Self {
        Self {
            tmp_dir: PathBuf::from("./tmp"),
        }
    }
}

impl Config {
    pub fn parse() -> Result<Self, config::ConfigError> {
        let args = cli::Args::parse();
        let mut builder = LibConfig::builder();
        if args.config_path.exists() {
            builder = builder.add_source(File::from(args.config_path));
        }
        builder
            .build()
            .expect("Failed to build config")
            .try_deserialize()
    }
}
