// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::env_merge;
use alloy::{
    network::Ethereum,
    primitives::{Address, ChainId},
    providers::DynProvider,
};
use anyhow::{Context, Result};
use interchain_indexer_entity::{
    bridge_contracts, bridges, chains, sea_orm_active_enums::BridgeType,
};
use interchain_indexer_logic::{NodeConfig, PoolConfig, build_layered_http_provider};
use sea_orm::{ActiveValue, entity::ActiveEnum};
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::HashMap, path::Path, str::FromStr, time::Duration};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexerType {
    IcmIctt,
    #[serde(rename = "amb")]
    #[allow(clippy::upper_case_acronyms)]
    AMB,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BridgeConfig {
    pub bridge_id: i32,
    pub name: String,
    #[serde(rename = "type", deserialize_with = "deserialize_bridge_type")]
    pub bridge_type: BridgeType,
    #[serde(default)]
    pub indexer_type: IndexerType,
    pub enabled: bool,
    pub api_url: Option<String>,
    pub ui_url: Option<String>,
    pub docs_url: Option<String>,
    /// When true, process messages involving at least one unknown chain
    /// (i.e. a chain not in `contracts`). When false (default), both endpoints
    /// must be configured chains.
    #[serde(default)]
    pub process_unknown_chains: bool,
    /// Optional chain id that narrows processing to messages where one endpoint
    /// equals this chain. Must be one of the chains configured in `contracts`.
    /// Validated at startup.
    #[serde(default)]
    pub home_chain_id: Option<ChainId>,
    pub contracts: Vec<BridgeContractConfig>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BridgeContractConfig {
    pub chain_id: i64,
    #[serde(deserialize_with = "deserialize_address")]
    pub address: Vec<u8>,
    pub version: i16,
    pub started_at_block: u64,
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "deserialize_abi")]
    pub abi: Option<String>,
}

/// Deserialize bridge type from JSON string using SeaORM ActiveEnum
fn deserialize_bridge_type<'de, D>(deserializer: D) -> Result<BridgeType, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    BridgeType::try_from_value(&s).map_err(serde::de::Error::custom)
}

/// Deserialize an ABI from either a JSON string (file form) or inline JSON
/// (env-override form), normalizing both to the string representation.
fn deserialize_abi<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(s),
        other => Some(other.to_string()),
    })
}

/// Deserialize an Ethereum address from a hex string to Vec<u8>
fn deserialize_address<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    // Parse address from hex string (with or without 0x prefix)
    let addr = Address::from_str(&s) //Address::from_str(&s).unwrap();
        .map_err(|e| serde::de::Error::custom(format!("Invalid address format: {}", e)))?;
    Ok(addr.as_slice().to_vec())
}

/// Convert BridgeConfig to bridges::ActiveModel for database operations
impl From<BridgeConfig> for bridges::ActiveModel {
    fn from(config: BridgeConfig) -> Self {
        bridges::ActiveModel {
            id: ActiveValue::Set(config.bridge_id),
            name: ActiveValue::Set(config.name),
            r#type: ActiveValue::Set(Some(config.bridge_type)),
            enabled: ActiveValue::Set(config.enabled),
            api_url: ActiveValue::Set(config.api_url),
            ui_url: ActiveValue::Set(config.ui_url),
            docs_url: ActiveValue::Set(config.docs_url),
            ..Default::default()
        }
    }
}

/// Convert bridges::Model to BridgeConfig
/// Note: This conversion loses the `indexer` field and `contracts` as they are not stored in the bridges table
impl From<bridges::Model> for BridgeConfig {
    fn from(model: bridges::Model) -> Self {
        BridgeConfig {
            bridge_id: model.id,
            name: model.name,
            bridge_type: model.r#type.expect("bridge must have a type"),
            indexer_type: Default::default(), // Not stored in database
            enabled: model.enabled,
            api_url: model.api_url,
            ui_url: model.ui_url,
            docs_url: model.docs_url,
            process_unknown_chains: false,
            home_chain_id: None,
            contracts: vec![], // Contracts are in a separate table
        }
    }
}

/// Convert BridgeContractConfig to bridge_contracts::ActiveModel for database operations
/// Note: `bridge_id` must be set separately as it's not part of BridgeContractConfig
impl BridgeContractConfig {
    pub fn to_active_model(&self, bridge_id: i32) -> bridge_contracts::ActiveModel {
        let abi_value = match &self.abi {
            None => None,
            Some(abi_str) => match serde_json::from_str::<serde_json::Value>(abi_str) {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(
                        err = %e,
                        abi_preview = %abi_str.chars().take(500).collect::<String>(),
                        "Invalid ABI JSON in bridge contract config, treating as None"
                    );
                    None
                }
            },
        };

        bridge_contracts::ActiveModel {
            bridge_id: ActiveValue::Set(bridge_id),
            chain_id: ActiveValue::Set(self.chain_id),
            address: ActiveValue::Set(self.address.clone()),
            version: ActiveValue::Set(self.version),
            kind: ActiveValue::Set(self.kind.clone()),
            started_at_block: ActiveValue::Set(Some(
                i64::try_from(self.started_at_block).expect("started_at_block must fit into i64"),
            )),
            abi: ActiveValue::Set(abi_value),
            ..Default::default()
        }
    }
}

/// Convert bridge_contracts::Model to BridgeContractConfig
/// Note: This conversion loses the `id` and `bridge_id` fields
impl From<bridge_contracts::Model> for BridgeContractConfig {
    fn from(model: bridge_contracts::Model) -> Self {
        let started_at_block = model.validated_started_at_block();
        let abi_string = model.abi.and_then(|json| serde_json::to_string(&json).ok());

        BridgeContractConfig {
            chain_id: model.chain_id,
            address: model.address,
            version: model.version,
            started_at_block,
            kind: model.kind,
            abi: abi_string,
        }
    }
}

/// Convert ChainConfig to chains::ActiveModel for database operations
impl From<ChainConfig> for chains::ActiveModel {
    fn from(config: ChainConfig) -> Self {
        // Build custom_routes JSON from ExplorerConfig fields
        let custom_routes = {
            let mut routes = serde_json::Map::new();
            if let Some(tx_route) = &config.explorer.custom_tx_route {
                routes.insert(
                    "tx".to_string(),
                    serde_json::Value::String(tx_route.clone()),
                );
            }
            if let Some(address_route) = &config.explorer.custom_address_route {
                routes.insert(
                    "address".to_string(),
                    serde_json::Value::String(address_route.clone()),
                );
            }
            if let Some(token_route) = &config.explorer.custom_token_route {
                routes.insert(
                    "token".to_string(),
                    serde_json::Value::String(token_route.clone()),
                );
            }
            if routes.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(routes))
            }
        };

        chains::ActiveModel {
            id: ActiveValue::Set(config.chain_id),
            name: ActiveValue::Set(config.name),
            icon: ActiveValue::Set(if config.icon.is_empty() {
                None
            } else {
                Some(config.icon)
            }),
            explorer: ActiveValue::Set(if config.explorer.url.is_empty() {
                None
            } else {
                Some(config.explorer.url)
            }),
            custom_routes: ActiveValue::Set(custom_routes),
            ..Default::default()
        }
    }
}

/// Convert chains::Model to ChainConfig
/// Note: This conversion loses the `rpcs` field as it's not stored in the chains table.
impl From<chains::Model> for ChainConfig {
    fn from(model: chains::Model) -> Self {
        // Extract custom routes from JSON
        let (custom_tx_route, custom_address_route, custom_token_route) =
            if let Some(routes) = &model.custom_routes {
                (
                    routes.get("tx").and_then(|v| v.as_str()).map(String::from),
                    routes
                        .get("address")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    routes
                        .get("token")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                )
            } else {
                (None, None, None)
            };

        ChainConfig {
            chain_id: model.id,
            name: model.name,
            icon: model.icon.unwrap_or_default(),
            explorer: ExplorerConfig {
                url: model.explorer.unwrap_or_default(),
                custom_tx_route,
                custom_address_route,
                custom_token_route,
            },
            pool_config: PoolConfig::default(),
            rpcs: vec![], // RPCs are not stored in database
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ExplorerConfig {
    #[serde(default)]
    pub url: String,
    pub custom_tx_route: Option<String>,
    pub custom_address_route: Option<String>,
    pub custom_token_route: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChainConfig {
    pub chain_id: i64,
    pub name: String,
    pub icon: String,
    #[serde(default)]
    pub explorer: ExplorerConfig,
    #[serde(default)]
    pub pool_config: PoolConfig,
    pub rpcs: Vec<HashMap<String, RpcProviderConfig>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RpcProviderConfig {
    pub url: String,
    #[serde(default = "default_rpc_enabled")]
    pub enabled: bool,
    #[serde(default = "default_max_rps")]
    max_rps: u32,
    #[serde(default = "default_error_threshold")]
    error_threshold: u32,
    #[serde(default = "default_cooldown_threshold")]
    cooldown_threshold: u32,
    #[serde(default = "default_cooldown_secs")]
    cooldown_secs: u64,
    #[serde(default = "default_multicall_batching_us")]
    multicall_batching_us: u64,
    #[serde(default)]
    pub api_key: Option<ApiKeyConfig>,
}

fn default_rpc_enabled() -> bool {
    true
}

fn default_max_rps() -> u32 {
    10
}

fn default_error_threshold() -> u32 {
    3
}

fn default_cooldown_threshold() -> u32 {
    1
}

fn default_cooldown_secs() -> u64 {
    60
}

fn default_multicall_batching_us() -> u64 {
    60
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApiKeyConfig {
    pub location: String,
    pub name: String,
}

/// Env var prefix for overriding/extending the chains config (see README).
const CHAINS_ENV_PREFIX: &str = "INTERCHAIN_INDEXER_CHAINS";
/// Env var prefix for overriding/extending the bridges config (see README).
const BRIDGES_ENV_PREFIX: &str = "INTERCHAIN_INDEXER_BRIDGES";

/// Load and deserialize chains from a JSON file, with
/// `INTERCHAIN_INDEXER_CHAINS*` env overrides deep-merged on top.
pub fn load_chains_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<ChainConfig>> {
    load_chains_impl(path, std::env::vars())
}

fn load_chains_impl<P: AsRef<Path>>(
    path: P,
    vars: impl Iterator<Item = (String, String)>,
) -> Result<Vec<ChainConfig>> {
    let mut value = read_config_array(path.as_ref(), "chains")?;

    let applied = env_merge::apply_env_overrides(
        &mut value,
        CHAINS_ENV_PREFIX,
        vars,
        &env_merge::CHAINS_RULES,
    )?;
    log_applied_overrides(&applied, "chains");

    serde_json::from_value(value).with_context(|| {
        format!(
            "Failed to parse chains config JSON (after env overrides): {:?}",
            path.as_ref()
        )
    })
}

/// Load and deserialize bridges from a JSON file, with
/// `INTERCHAIN_INDEXER_BRIDGES*` env overrides deep-merged on top.
pub fn load_bridges_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<BridgeConfig>> {
    load_bridges_impl(path, std::env::vars())
}

fn load_bridges_impl<P: AsRef<Path>>(
    path: P,
    vars: impl Iterator<Item = (String, String)>,
) -> Result<Vec<BridgeConfig>> {
    let mut value = read_config_array(path.as_ref(), "bridges")?;

    let applied = env_merge::apply_env_overrides(
        &mut value,
        BRIDGES_ENV_PREFIX,
        vars,
        &env_merge::BRIDGES_RULES,
    )?;
    log_applied_overrides(&applied, "bridges");

    serde_json::from_value(value).with_context(|| {
        format!(
            "Failed to parse bridges config JSON (after env overrides): {:?}",
            path.as_ref()
        )
    })
}

fn log_applied_overrides(applied: &[env_merge::AppliedOverride], kind: &str) {
    for o in applied {
        // No raw config values at info level: RPC URLs and similar fields may
        // embed API keys. Replaced fields are identified by path at info;
        // the old/new values are available at debug for troubleshooting.
        tracing::info!(var = %o.var, path = %o.json_path, kind, "applied config env override");
        for overwrite in &o.overwrites {
            tracing::info!(
                var = %o.var,
                path = %overwrite.path,
                kind,
                "config env override replaced an existing value"
            );
            tracing::debug!(
                var = %o.var,
                path = %overwrite.path,
                old = %overwrite.old,
                new = %overwrite.new,
                kind,
                "config env override replacement values"
            );
        }
    }
}

fn read_config_array(path: &Path, kind: &str) -> Result<serde_json::Value> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {kind} config file: {path:?}"))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {kind} config JSON: {path:?}"))?;
    anyhow::ensure!(
        value.is_array(),
        "{kind} config must be a JSON array: {path:?}"
    );
    Ok(value)
}

/// Create layered Alloy providers from ChainConfig definitions.
/// Returns a HashMap mapping chain_id (as i64) to a DynProvider.
/// Only enabled RPC providers are included in each pool.
pub async fn create_provider_pools_from_chains(
    chains: Vec<ChainConfig>,
) -> Result<HashMap<i64, DynProvider<Ethereum>>> {
    let mut pools = HashMap::new();

    for chain in chains {
        if chain.chain_id < 0 {
            tracing::warn!(
                chain_id = chain.chain_id,
                chain_name = chain.name,
                "Skipping chain with negative ID"
            );
            continue;
        }

        let mut node_configs = Vec::new();

        // Extract enabled RPC providers from the chain config
        for rpc_map in &chain.rpcs {
            for (provider_name, rpc_config) in rpc_map {
                // Only include enabled providers
                if !rpc_config.enabled {
                    continue;
                }

                // Build the URL (handle API key placeholders if needed)
                let url = build_rpc_url(&rpc_config.url, &rpc_config.api_key)?;

                let node_config = NodeConfig {
                    name: format!("{}[{}]", chain.name, provider_name),
                    http_url: url,
                    max_rps: rpc_config.max_rps,
                    error_threshold: rpc_config.error_threshold,
                    cooldown_threshold: rpc_config.cooldown_threshold,
                    cooldown: Duration::from_secs(rpc_config.cooldown_secs),
                    multicall_batching_wait: Duration::from_micros(
                        rpc_config.multicall_batching_us,
                    ),
                };

                node_configs.push(node_config);
            }
        }

        // Create layered provider for this chain if we have any nodes
        if !node_configs.is_empty() {
            // Check for duplicate chain_id in config
            if pools.contains_key(&chain.chain_id) {
                anyhow::bail!("Duplicate chain_id {} in chains config", chain.chain_id,);
            }

            match build_layered_http_provider(node_configs, chain.pool_config.clone()) {
                Ok(provider) => {
                    tracing::info!(
                        chain_id = chain.chain_id,
                        chain_name = chain.name,
                        "Created layered provider for chain"
                    );
                    pools.insert(chain.chain_id, provider);
                }
                Err(e) => {
                    tracing::warn!(
                        chain_id = chain.chain_id,
                        chain_name = chain.name,
                        err = ?e,
                        "Failed to create layered provider for chain, skipping"
                    );
                }
            }
        } else {
            tracing::warn!(
                chain_id = chain.chain_id,
                chain_name = chain.name,
                "No enabled RPC providers found for chain, skipping provider creation"
            );
        }
    }

    Ok(pools)
}

/// Build RPC URL, handling API key placeholders if present.
/// Note: This is a simplified implementation. In production, you might want to:
/// - Read API keys from environment variables
/// - Support different API key locations (query, header, URL)
fn build_rpc_url(url: &str, api_key_config: &Option<ApiKeyConfig>) -> Result<String> {
    let final_url = url.to_string();

    // If API key is configured, we need to handle it
    // For now, we'll just use the URL as-is and log a warning if API key is needed
    if let Some(api_key) = api_key_config {
        anyhow::bail!(
            "API key config ({}/{}) present for {url} but substitution is not implemented",
            api_key.location,
            api_key.name
        );
    }

    Ok(final_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_bridge_without_home_chain_field() {
        let json = r#"
        [
            {
                "bridge_id": 7,
                "name": "No Home Chain",
                "type": "avalanche_native",
                "indexer_type": "icm_ictt",
                "enabled": true,
                "api_url": null,
                "ui_url": null,
                "docs_url": null,
                "contracts": []
            }
        ]
        "#;

        let bridges: Vec<BridgeConfig> = serde_json::from_str(json).unwrap();
        assert_eq!(bridges.len(), 1);
        assert!(!bridges[0].process_unknown_chains);
        assert_eq!(bridges[0].home_chain_id, None);
    }

    #[test]
    fn test_deserialize_bridge_with_home_chain_id_field() {
        let json = r#"
        [
            {
                "bridge_id": 7,
                "name": "With Home Chain",
                "type": "avalanche_native",
                "indexer_type": "icm_ictt",
                "enabled": true,
                "api_url": null,
                "ui_url": null,
                "docs_url": null,
                "process_unknown_chains": true,
                "home_chain_id": 43114,
                "contracts": []
            }
        ]
        "#;

        let bridges: Vec<BridgeConfig> = serde_json::from_str(json).unwrap();
        assert_eq!(bridges.len(), 1);
        assert!(bridges[0].process_unknown_chains);
        assert_eq!(bridges[0].home_chain_id, Some(43114));
    }

    #[test]
    fn test_model_to_bridge_config() {
        use interchain_indexer_entity::bridges;

        let model = bridges::Model {
            id: 1,
            name: "Test Bridge".to_string(),
            r#type: Some(BridgeType::Lockmint),
            enabled: true,
            api_url: Some("https://api.example.com".to_string()),
            ui_url: Some("https://ui.example.com".to_string()),
            docs_url: Some("https://docs.example.com".to_string()),
            created_at: None,
            updated_at: None,
        };

        let config: BridgeConfig = model.into();

        assert_eq!(config.bridge_id, 1);
        assert_eq!(config.name, "Test Bridge");
        assert_eq!(config.bridge_type, BridgeType::Lockmint);
        assert!(config.enabled);
        assert_eq!(config.api_url, Some("https://api.example.com".to_string()));
        assert_eq!(config.ui_url, Some("https://ui.example.com".to_string()));
        assert_eq!(
            config.docs_url,
            Some("https://docs.example.com".to_string())
        );
        // indexer and contracts are lost in conversion (not stored in DB)
        assert_eq!(config.indexer_type, IndexerType::Unknown);
        assert!(!config.process_unknown_chains);
        assert_eq!(config.home_chain_id, None);
        assert_eq!(config.contracts, vec![]);
    }

    #[test]
    fn test_bridge_contract_config_to_active_model() {
        let config = BridgeContractConfig {
            chain_id: 1,
            address: vec![0x12; 20],
            version: 1,
            started_at_block: 12345,
            kind: None,
            abi: None,
        };

        let active_model = config.to_active_model(100);

        assert!(matches!(active_model.bridge_id, ActiveValue::Set(100)));
        assert!(matches!(active_model.chain_id, ActiveValue::Set(1)));
        assert!(matches!(active_model.version, ActiveValue::Set(1)));
        assert!(matches!(
            active_model.started_at_block,
            ActiveValue::Set(Some(12345))
        ));
    }

    #[test]
    fn test_model_to_bridge_contract_config() {
        use interchain_indexer_entity::bridge_contracts;

        let model = bridge_contracts::Model {
            id: 1,
            bridge_id: 100,
            chain_id: 1,
            address: vec![0x12; 20],
            version: 1,
            abi: None,
            started_at_block: Some(12345),
            created_at: None,
            updated_at: None,
            kind: None,
        };

        let config: BridgeContractConfig = model.into();

        assert_eq!(config.chain_id, 1);
        assert_eq!(config.address, vec![0x12; 20]);
        assert_eq!(config.version, 1);
        assert_eq!(config.started_at_block, 12345);
    }

    #[test]
    fn test_model_to_bridge_contract_config_clamps_started_at_block() {
        use interchain_indexer_entity::bridge_contracts;

        let common = bridge_contracts::Model {
            id: 1,
            bridge_id: 100,
            chain_id: 1,
            address: vec![0x12; 20],
            version: 1,
            abi: None,
            created_at: None,
            updated_at: None,
            started_at_block: None,
            kind: None,
        };

        let none_block: BridgeContractConfig = common.clone().into();
        assert_eq!(none_block.started_at_block, 0);

        let negative_block: BridgeContractConfig = bridge_contracts::Model {
            started_at_block: Some(-42),
            ..common
        }
        .into();
        assert_eq!(negative_block.started_at_block, 0);
    }

    #[test]
    fn test_chain_config_to_active_model() {
        let config = ChainConfig {
            chain_id: 1,
            name: "Ethereum".to_string(),
            icon: "https://example.com/icon.png".to_string(),
            explorer: ExplorerConfig {
                url: "https://etherscan.io".to_string(),
                custom_tx_route: Some("/transaction/{hash}".to_string()),
                custom_address_route: None,
                custom_token_route: None,
            },
            pool_config: Default::default(),
            rpcs: vec![],
        };

        let active_model: chains::ActiveModel = config.clone().into();

        assert!(matches!(active_model.id, ActiveValue::Set(1)));
        assert!(matches!(active_model.name, ActiveValue::Set(ref name) if name == "Ethereum"));
        assert!(matches!(
            active_model.icon,
            ActiveValue::Set(Some(ref icon)) if icon == "https://example.com/icon.png"
        ));
        assert!(matches!(
            active_model.explorer,
            ActiveValue::Set(Some(ref url)) if url == "https://etherscan.io"
        ));
        // Check custom_routes contains the tx route
        if let ActiveValue::Set(Some(ref routes)) = active_model.custom_routes {
            assert_eq!(
                routes.get("tx").and_then(|v| v.as_str()),
                Some("/transaction/{hash}")
            );
            assert!(routes.get("address").is_none());
        } else {
            panic!("Expected custom_routes to be set");
        }
    }

    #[test]
    fn test_chain_config_to_active_model_empty_icon() {
        let config = ChainConfig {
            chain_id: 1,
            name: "Ethereum".to_string(),
            icon: String::new(),
            explorer: ExplorerConfig {
                url: String::new(),
                custom_tx_route: None,
                custom_address_route: None,
                custom_token_route: None,
            },
            pool_config: Default::default(),
            rpcs: vec![],
        };

        let active_model: chains::ActiveModel = config.into();

        assert!(matches!(active_model.icon, ActiveValue::Set(None)));
        assert!(matches!(active_model.explorer, ActiveValue::Set(None)));
        assert!(matches!(active_model.custom_routes, ActiveValue::Set(None)));
    }

    #[test]
    fn test_model_to_chain_config() {
        use interchain_indexer_entity::chains;

        let custom_routes = serde_json::json!({
            "tx": "/transaction/{hash}",
            "address": "/addr/{hash}"
        });

        let model = chains::Model {
            id: 1,
            name: "Ethereum".to_string(),
            icon: Some("https://example.com/icon.png".to_string()),
            explorer: Some("https://etherscan.io".to_string()),
            custom_routes: Some(custom_routes),
            created_at: None,
            updated_at: None,
        };

        let config: ChainConfig = model.into();

        assert_eq!(config.chain_id, 1);
        assert_eq!(config.name, "Ethereum");
        assert_eq!(config.icon, "https://example.com/icon.png");
        assert_eq!(config.explorer.url, "https://etherscan.io");
        assert_eq!(
            config.explorer.custom_tx_route,
            Some("/transaction/{hash}".to_string())
        );
        assert_eq!(
            config.explorer.custom_address_route,
            Some("/addr/{hash}".to_string())
        );
        assert_eq!(config.explorer.custom_token_route, None);
        // rpcs are lost in conversion (not stored in DB)
        assert_eq!(config.rpcs, vec![]);
    }

    #[test]
    fn test_deserialize_abi_accepts_string_and_inline_json_forms() {
        let file_form = r#"
        {
            "chain_id": 1,
            "address": "0x4C36d2919e407f0Cc2Ee3c993ccF8ac26d9CE64e",
            "version": 6,
            "started_at_block": 1,
            "kind": null,
            "abi": "[{\"name\":\"RelayedMessage\",\"type\":\"event\"}]"
        }
        "#;
        let env_form = r#"
        {
            "chain_id": 1,
            "address": "0x4C36d2919e407f0Cc2Ee3c993ccF8ac26d9CE64e",
            "version": 6,
            "started_at_block": 1,
            "kind": null,
            "abi": [{"name":"RelayedMessage","type":"event"}]
        }
        "#;

        let from_file: BridgeContractConfig = serde_json::from_str(file_form).unwrap();
        let from_env: BridgeContractConfig = serde_json::from_str(env_form).unwrap();

        assert_eq!(
            from_file.abi,
            Some(r#"[{"name":"RelayedMessage","type":"event"}]"#.to_string())
        );
        assert_eq!(from_file.abi, from_env.abi);

        let null_form = file_form.replace(
            r#""abi": "[{\"name\":\"RelayedMessage\",\"type\":\"event\"}]""#,
            r#""abi": null"#,
        );
        let from_null: BridgeContractConfig = serde_json::from_str(&null_form).unwrap();
        assert_eq!(from_null.abi, None);
    }

    fn write_temp_json(content: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    fn fixture_vars(vars: &[(&str, &str)]) -> impl Iterator<Item = (String, String)> {
        vars.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>()
            .into_iter()
    }

    const CHAINS_FILE: &str = r#"
    [
        {
            "chain_id": 1,
            "name": "Ethereum",
            "icon": "https://icon.example/eth.svg",
            "rpcs": [ { "drpc": { "url": "https://eth.drpc.org" } } ]
        }
    ]
    "#;

    const BRIDGES_FILE: &str = r#"
    [
        {
            "bridge_id": 1,
            "name": "AMB",
            "type": "amb",
            "indexer_type": "amb",
            "enabled": true,
            "api_url": "https://api.example",
            "ui_url": null,
            "docs_url": null,
            "contracts": [
                {
                    "chain_id": 100,
                    "address": "0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d",
                    "version": 6,
                    "started_at_block": 10
                }
            ]
        }
    ]
    "#;

    #[test]
    fn test_load_chains_impl_without_override_vars_matches_file() {
        let file = write_temp_json(CHAINS_FILE);
        let chains = load_chains_impl(file.path(), fixture_vars(&[])).unwrap();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].chain_id, 1);
        assert_eq!(chains[0].name, "Ethereum");
    }

    #[test]
    fn test_load_chains_impl_new_chain_field_by_field_parses_typed() {
        let file = write_temp_json(CHAINS_FILE);
        let chains = load_chains_impl(
            file.path(),
            fixture_vars(&[
                ("INTERCHAIN_INDEXER_CHAINS__137__NAME", "Polygon"),
                (
                    "INTERCHAIN_INDEXER_CHAINS__137__ICON",
                    "https://icon.example/poly.svg",
                ),
                (
                    "INTERCHAIN_INDEXER_CHAINS__137__RPCS__MYNODE__URL",
                    "https://my.node",
                ),
            ]),
        )
        .unwrap();

        assert_eq!(chains.len(), 2);
        let polygon = &chains[1];
        assert_eq!(polygon.chain_id, 137);
        assert_eq!(polygon.name, "Polygon");
        assert_eq!(polygon.rpcs[0]["mynode"].url, "https://my.node");
    }

    #[test]
    fn test_load_bridges_impl_null_api_url_parses_as_none() {
        let file = write_temp_json(BRIDGES_FILE);
        let bridges = load_bridges_impl(
            file.path(),
            fixture_vars(&[("INTERCHAIN_INDEXER_BRIDGES__1__API_URL", "null")]),
        )
        .unwrap();

        assert_eq!(bridges[0].api_url, None);
    }

    #[test]
    fn test_load_bridges_impl_new_bridge_fragment_parses_typed() {
        let file = write_temp_json(BRIDGES_FILE);
        let bridges = load_bridges_impl(
            file.path(),
            fixture_vars(&[(
                "INTERCHAIN_INDEXER_BRIDGES__2",
                r#"{
                    "name": "Avalanche ICTT",
                    "type": "avalanche_native",
                    "indexer_type": "icm_ictt",
                    "enabled": false,
                    "api_url": null,
                    "ui_url": null,
                    "docs_url": null,
                    "contracts": [
                        {
                            "chain_id": 43114,
                            "address": "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf",
                            "version": 1,
                            "started_at_block": 42526120
                        }
                    ]
                }"#,
            )]),
        )
        .unwrap();

        assert_eq!(bridges.len(), 2);
        let new_bridge = &bridges[1];
        assert_eq!(new_bridge.bridge_id, 2);
        assert_eq!(new_bridge.indexer_type, IndexerType::IcmIctt);
        assert!(!new_bridge.enabled);
        assert_eq!(new_bridge.contracts.len(), 1);
        assert_eq!(new_bridge.contracts[0].chain_id, 43114);
    }

    #[test]
    fn test_load_chains_impl_env_built_chain_missing_name_errors() {
        let file = write_temp_json(CHAINS_FILE);
        let err = load_chains_impl(
            file.path(),
            fixture_vars(&[(
                "INTERCHAIN_INDEXER_CHAINS__137__ICON",
                "https://icon.example/poly.svg",
            )]),
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("name"), "unexpected: {err:#}");
    }

    #[test]
    fn test_load_chains_impl_unknown_field_in_env_path_errors() {
        let file = write_temp_json(CHAINS_FILE);
        let err = load_chains_impl(
            file.path(),
            fixture_vars(&[("INTERCHAIN_INDEXER_CHAINS__1__NAME_TYPO", "X")]),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("name_typo"),
            "unexpected: {err:#}"
        );
    }

    #[test]
    fn test_load_bridges_impl_inline_json_abi_from_env_parses() {
        let file = write_temp_json(BRIDGES_FILE);
        let bridges = load_bridges_impl(
            file.path(),
            fixture_vars(&[(
                "INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0xF6A78083CA3E2A662D6DD1703C939C8ACE2E268D__6__ABI",
                r#"[{"name":"RelayedMessage","type":"event"}]"#,
            )]),
        )
        .unwrap();

        assert_eq!(
            bridges[0].contracts[0].abi,
            Some(r#"[{"name":"RelayedMessage","type":"event"}]"#.to_string())
        );
    }

    /// Collect all `.json` files under a directory, recursively.
    fn collect_json_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_json_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "json") {
                out.push(path);
            }
        }
    }

    #[test]
    fn test_all_repo_config_files_parse_through_strict_structs() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        collect_json_files(&repo_root.join("config"), &mut files);
        collect_json_files(&repo_root.join("docker/config"), &mut files);
        assert!(!files.is_empty(), "no config JSON files found");

        for path in files {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let content = std::fs::read_to_string(&path).unwrap();
            if name.starts_with("chains") {
                serde_json::from_str::<Vec<ChainConfig>>(&content)
                    .unwrap_or_else(|e| panic!("failed to parse {path:?} as chains config: {e}"));
            } else if name.starts_with("bridges") {
                serde_json::from_str::<Vec<BridgeConfig>>(&content)
                    .unwrap_or_else(|e| panic!("failed to parse {path:?} as bridges config: {e}"));
            } else {
                panic!("unexpected config file {path:?}: neither chains* nor bridges*");
            }
        }
    }
}
