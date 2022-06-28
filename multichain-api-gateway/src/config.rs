use serde::Deserialize;
use std::net::SocketAddr;
use url::Url;

/// An instance of the maintained networks in Blockscout.
/// Semantic: (network, chain)
/// e.g."blockscout.com/eth/mainnet" -> ("eth", "mainnet")
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Instance(pub String, pub String);

/// Settings for the Blockscout API
#[derive(Deserialize, Clone)]
pub struct BlockScoutSettings {
    /// The base URL of the Blockscout API.
    /// At the moment, should be equal to "https://blockscout.com"
    pub base_url: Url,

    pub instances: Vec<Instance>,

    /// The number of concurrent requests to be made to the Blockscout API from a server thread worker.
    pub concurrent_requests: usize,
}

#[derive(Deserialize, Clone)]
pub struct ServerSettings {
    pub addr: SocketAddr,
}

#[derive(Deserialize, Clone)]
pub struct Settings {
    pub server: ServerSettings,
    pub blockscout: BlockScoutSettings,
}

pub fn get_config() -> std::io::Result<Settings> {
    let content = std::fs::read_to_string("../example_config.toml")?;
    Ok(toml::from_str(&content)?)
}
