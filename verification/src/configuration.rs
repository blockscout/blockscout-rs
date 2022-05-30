use config::{Config, File, FileFormat};
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Deserialize, Clone)]
pub struct Configuration {
    pub server: ServerConfiguration,
    pub urls: UrlsConfiguration,
}

#[derive(Deserialize, Clone)]
pub struct ServerConfiguration {
    pub addr: SocketAddr,
}

#[derive(Deserialize, Clone)]
pub struct UrlsConfiguration {
    pub sourcify_api: String,
}

impl Configuration {
    pub fn parse() -> Result<Self, config::ConfigError> {
        let builder = Config::builder()
            .add_source(File::new("configuration", FileFormat::Toml))
            .add_source(config::Environment::with_prefix("VERIFICATION"))
            .build()
            .expect("Failed to build config");

        builder.try_deserialize()
    }
}
