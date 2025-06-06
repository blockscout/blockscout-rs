use bens_logic::protocols::{AddressResolveTechnique, ProtocolMeta, ProtocolSpecific, Tld};
use blockscout_service_launcher::{
    database::{DatabaseConnectSettings, DatabaseSettings},
    launcher::{ConfigSettings, MetricsSettings, ServerSettings},
    tracing::{JaegerSettings, TracingSettings},
};
use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub metrics: MetricsSettings,
    #[serde(default)]
    pub tracing: TracingSettings,
    #[serde(default)]
    pub jaeger: JaegerSettings,
    #[serde(default)]
    pub subgraphs_reader: SubgraphsReaderSettings,
    pub database: DatabaseSettings,
    #[serde(default = "default_swagger_path")]
    pub swagger_path: PathBuf,
}

impl ConfigSettings for Settings {
    const SERVICE_NAME: &'static str = "BENS";
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SubgraphsReaderSettings {
    #[serde(default)]
    pub protocols: HashMap<String, ProtocolSettings>,
    #[serde(default)]
    pub networks: HashMap<i64, NetworkSettings>,
    #[serde(default = "default_refresh_cache_schedule")]
    pub refresh_cache_schedule: String,
}

fn default_refresh_cache_schedule() -> String {
    "0 0 * * * *".to_string() // every hour
}

impl Default for SubgraphsReaderSettings {
    fn default() -> Self {
        Self {
            networks: Default::default(),
            protocols: Default::default(),
            refresh_cache_schedule: default_refresh_cache_schedule(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ProtocolSettings {
    #[serde(default)]
    pub disabled: bool,
    pub tld_list: NonEmpty<Tld>,
    pub network_id: i64,
    pub subgraph_name: String,
    #[serde(default)]
    pub address_resolve_technique: AddressResolveTechnique,
    #[serde(default)]
    pub meta: ProtocolSettingsMeta,
    #[serde(default, rename = "specific")]
    pub protocol_specific: ProtocolSettingsSpecific,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProtocolSettingsMeta(pub ProtocolMeta);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProtocolSettingsSpecific(pub ProtocolSpecific);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct NetworkSettings {
    pub blockscout: BlockscoutSettings,
    #[serde(default)]
    pub use_protocols: Vec<String>,
    #[serde(default)]
    pub rpc_url: Option<Url>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, default)]
pub struct BlockscoutSettings {
    pub url: Url,
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
    #[serde(default = "default_blockscout_timeout")]
    pub timeout: u64,
}

fn default_blockscout_url() -> Url {
    "http://localhost:4000".parse().unwrap()
}

fn default_max_concurrent_requests() -> usize {
    5
}

fn default_blockscout_timeout() -> u64 {
    30
}

impl Default for BlockscoutSettings {
    fn default() -> Self {
        Self {
            url: default_blockscout_url(),
            max_concurrent_requests: default_max_concurrent_requests(),
            timeout: default_blockscout_timeout(),
        }
    }
}

fn default_swagger_path() -> PathBuf {
    blockscout_endpoint_swagger::default_swagger_path_from_service_name("bens")
}

impl Settings {
    pub fn default(database_url: String) -> Self {
        Self {
            server: Default::default(),
            metrics: Default::default(),
            tracing: Default::default(),
            jaeger: Default::default(),
            subgraphs_reader: Default::default(),
            database: DatabaseSettings {
                connect: DatabaseConnectSettings::Url(database_url),
                connect_options: Default::default(),
                create_database: Default::default(),
                run_migrations: Default::default(),
            },
            swagger_path: default_swagger_path(),
        }
    }
}
