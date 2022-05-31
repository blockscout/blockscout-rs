use config::{Config as LibConfig, File, FileFormat};
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfiguration,
    pub sourcify: SourcifyConfiguration,
}

#[derive(Deserialize, Clone)]
pub struct ServerConfiguration {
    pub addr: SocketAddr,
}

#[derive(Deserialize, Clone)]
pub struct SourcifyConfiguration {
    pub api_url: String,
}

impl Config {
    pub fn parse() -> Result<Self, config::ConfigError> {
        let builder = LibConfig::builder()
            .add_source(File::new("configuration", FileFormat::Toml))
            .add_source(config::Environment::with_prefix("VERIFICATION"))
            .build()
            .expect("Failed to build config");

        builder.try_deserialize()
    }
}
