use std::net::SocketAddr;

use url::Url;

#[derive(serde::Deserialize, Clone)]
pub struct Settings {
    pub server: ServerSettings,
    pub blockscout: BlockScoutSettings,
}

#[derive(serde::Deserialize, Clone)]
pub struct ServerSettings {
    pub addr: SocketAddr,
}

#[derive(serde::Deserialize, Clone)]
pub struct Instance(pub String, pub String);

#[derive(serde::Deserialize, Clone)]
pub struct BlockScoutSettings {
    pub base_url: Url,
    pub instances: Vec<Instance>,
    pub concurrent_requests: usize,
}

pub fn get_config() -> std::io::Result<Settings> {
    let content = std::fs::read_to_string("unification/config.toml")?;
    Ok(toml::from_str(&content)?)
}
