use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub addr: SocketAddr,
}
