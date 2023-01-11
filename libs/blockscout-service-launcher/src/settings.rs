use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, str::FromStr};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSettings {
    pub http: HttpServerSettings,
    pub grpc: GrpcServerSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpServerSettings {
    pub enabled: bool,
    pub addr: SocketAddr,
}

impl Default for HttpServerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            addr: SocketAddr::from_str("0.0.0.0:8043").unwrap(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GrpcServerSettings {
    pub enabled: bool,
    pub addr: SocketAddr,
}

impl Default for GrpcServerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            addr: SocketAddr::from_str("0.0.0.0:8044").unwrap(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsSettings {
    pub enabled: bool,
    pub addr: SocketAddr,
    pub route: String,
}

impl Default for MetricsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            addr: SocketAddr::from_str("0.0.0.0:6060").expect("should be valid url"),
            route: "/metrics".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TracingSettings {
    /// If disabled, tracing is not initialized for neither
    /// stdout, nor jaeger (enabled by default).
    pub enabled: bool,
}

impl Default for TracingSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JaegerSettings {
    pub enabled: bool,
    pub agent_endpoint: String,
}

impl Default for JaegerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            agent_endpoint: "127.0.0.1:6831".to_string(),
        }
    }
}
