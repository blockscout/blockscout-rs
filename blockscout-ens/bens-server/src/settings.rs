// SPDX-License-Identifier: LicenseRef-Blockscout

use bens_logic::protocols::{AddressResolveTechnique, ProtocolMeta, ProtocolSpecific, Tld};
use blockscout_service_launcher::{
    database::{
        DatabaseConnectOptionsSettings, DatabaseConnectSettings, DatabaseSettings,
        ReplicaDatabaseSettings,
    },
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
    #[serde(default)]
    pub replica_database: Option<ReplicaDatabaseSettings>,
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
    #[serde(default)]
    pub refresh_cache_disabled: bool,
    /// Maximum number of protocols a user may specify in a single protocol-scoped
    /// request (chain id omitted). `None` disables the limit.
    #[serde(default = "default_max_protocols_from_user_input")]
    pub max_protocols_from_user_input: Option<usize>,
}

fn default_refresh_cache_schedule() -> String {
    "0 0 * * * *".to_string() // every hour
}

fn default_max_protocols_from_user_input() -> Option<usize> {
    None
}

impl Default for SubgraphsReaderSettings {
    fn default() -> Self {
        Self {
            networks: Default::default(),
            protocols: Default::default(),
            refresh_cache_schedule: default_refresh_cache_schedule(),
            refresh_cache_disabled: false,
            max_protocols_from_user_input: default_max_protocols_from_user_input(),
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
    pub forward_resolution_grace_period_seconds: u64,
    #[serde(default)]
    pub primary_name_record_requires_active_owner: bool,
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
                connect_options: DatabaseConnectOptionsSettings {
                    postgres_application_name: Some("BENS".into()),
                    postgres_statement_timeout: Some("60s".into()),
                    ..Default::default()
                },
                create_database: Default::default(),
                run_migrations: Default::default(),
            },
            replica_database: Default::default(),
            swagger_path: default_swagger_path(),
        }
    }
}

#[cfg(test)]
mod rensa_config_tests {
    use super::*;

    #[test]
    fn production_rensa_config_parses_without_an_ens_registry() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../config/prod.json")).unwrap();
        let reader = &config["subgraphs_reader"];
        let protocol: ProtocolSettings =
            serde_json::from_value(reader["protocols"]["rensa"].clone()).unwrap();
        let network: NetworkSettings =
            serde_json::from_value(reader["networks"]["4663"].clone()).unwrap();

        assert_eq!(protocol.network_id, 4663);
        assert_eq!(protocol.subgraph_name, "rensa-subgraph");
        assert_eq!(protocol.tld_list.head, Tld::new("rns"));
        assert_eq!(protocol.forward_resolution_grace_period_seconds, 7_776_000);
        assert!(protocol.primary_name_record_requires_active_owner);
        assert_eq!(
            protocol.address_resolve_technique,
            AddressResolveTechnique::PrimaryNameRecord
        );
        assert_eq!(network.use_protocols, vec!["rensa".to_string()]);
        match protocol.protocol_specific.0 {
            ProtocolSpecific::EnsLike(ens) => {
                assert_eq!(
                    ens.native_token_contract,
                    Some(
                        "0x08ed77b2ec313c7ad5ce23747b07d483071485f9"
                            .parse()
                            .unwrap()
                    )
                );
                assert!(ens.registry_contract.is_none());
                assert!(!ens.try_offchain_resolve);
            }
            _ => panic!("Rensa must use the BENS index-hashing codec only"),
        }
    }

    #[test]
    fn legacy_protocol_config_defaults_to_no_grace_or_primary_guard() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../config/prod.json")).unwrap();
        let protocols = config["subgraphs_reader"]["protocols"].as_object().unwrap();
        for (slug, value) in protocols {
            if slug == "rensa" {
                continue;
            }
            let protocol: ProtocolSettings = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(
                protocol.forward_resolution_grace_period_seconds, 0,
                "{slug}"
            );
            assert!(
                !protocol.primary_name_record_requires_active_owner,
                "{slug}"
            );
        }
    }
}
