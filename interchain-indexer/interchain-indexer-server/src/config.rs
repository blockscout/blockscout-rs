use alloy::{network::Ethereum, primitives::Address, providers::DynProvider};
use anyhow::{Context, Result};
use interchain_indexer_entity::{
    bridge_contracts, bridges, chains, sea_orm_active_enums::BridgeType,
};
use interchain_indexer_logic::{NodeConfig, PoolConfig, build_layered_http_provider};
use sea_orm::{ActiveValue, entity::ActiveEnum};
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, path::Path, str::FromStr, time::Duration};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BridgeConfig {
    pub bridge_id: i32,
    pub name: String,
    #[serde(rename = "type")]
    pub bridge_type: String,
    pub indexer: String,
    pub enabled: bool,
    pub api_url: Option<String>,
    pub ui_url: Option<String>,
    pub docs_url: Option<String>,
    pub contracts: Vec<BridgeContractConfig>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BridgeContractConfig {
    pub chain_id: i64,
    #[serde(deserialize_with = "deserialize_address")]
    pub address: Vec<u8>,
    pub version: i16,
    pub started_at_block: i64,
    pub abi: Option<String>,
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
        let bridge_type = match BridgeType::try_from_value(&config.bridge_type) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!(
                    bridge_type = %config.bridge_type,
                    err = ?e,
                    "Unknown bridge type in config; storing as NULL"
                );
                None
            }
        };
        bridges::ActiveModel {
            id: ActiveValue::Set(config.bridge_id),
            name: ActiveValue::Set(config.name),
            r#type: ActiveValue::Set(bridge_type),
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
            bridge_type: model.r#type.map(|t| t.to_value()).unwrap_or_default(),
            indexer: String::new(), // Not stored in database
            enabled: model.enabled,
            api_url: model.api_url,
            ui_url: model.ui_url,
            docs_url: model.docs_url,
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
            started_at_block: ActiveValue::Set(Some(self.started_at_block)),
            abi: ActiveValue::Set(abi_value),
            ..Default::default()
        }
    }
}

/// Convert bridge_contracts::Model to BridgeContractConfig
/// Note: This conversion loses the `id` and `bridge_id` fields
impl From<bridge_contracts::Model> for BridgeContractConfig {
    fn from(model: bridge_contracts::Model) -> Self {
        let abi_string = model.abi.and_then(|json| serde_json::to_string(&json).ok());

        BridgeContractConfig {
            chain_id: model.chain_id,
            address: model.address,
            version: model.version,
            started_at_block: model.started_at_block.unwrap_or(0),
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
/// Note: This conversion loses the `rpcs` field as it's not stored in the chains table
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
            rpcs: vec![], // RPCs are not stored in database
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct ExplorerConfig {
    #[serde(default)]
    pub url: String,
    pub custom_tx_route: Option<String>,
    pub custom_address_route: Option<String>,
    pub custom_token_route: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ChainConfig {
    pub chain_id: i64,
    pub name: String,
    pub icon: String,
    #[serde(default)]
    pub explorer: ExplorerConfig,
    pub rpcs: Vec<HashMap<String, RpcProviderConfig>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RpcProviderConfig {
    pub url: String,
    #[serde(default = "default_rpc_enabled")]
    pub enabled: bool,
    #[serde(default = "default_max_rps")]
    max_rps: u32,
    #[serde(default = "default_error_threshold")]
    error_threshold: u32,
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

fn default_multicall_batching_us() -> u64 {
    60
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ApiKeyConfig {
    pub location: String,
    pub name: String,
}

/// Load and deserialize chains from a JSON file
pub fn load_chains_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<ChainConfig>> {
    let content = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("Failed to read chains config file: {:?}", path.as_ref()))?;

    let chains: Vec<ChainConfig> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse chains config JSON: {:?}", path.as_ref()))?;

    Ok(chains)
}

/// Load and deserialize bridges from a JSON file
pub fn load_bridges_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<BridgeConfig>> {
    let content = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("Failed to read bridges config file: {:?}", path.as_ref()))?;

    let bridges: Vec<BridgeConfig> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse bridges config JSON: {:?}", path.as_ref()))?;

    Ok(bridges)
}

/// Create layered Alloy providers from ChainConfig definitions.
/// Returns a HashMap mapping chain_id (as i64) to a DynProvider.
/// Only enabled RPC providers are included in each pool.
pub async fn create_provider_pools_from_chains(
    chains: Vec<ChainConfig>,
) -> Result<HashMap<i64, DynProvider<Ethereum>>> {
    let mut pools = HashMap::new();

    // Default pool configuration
    let pool_config = PoolConfig {
        health_period: Duration::from_secs(1),
        max_block_lag: 100,
        retry_count: 3,
        retry_initial_delay_ms: 5,
        retry_max_delay_ms: 100,
    };

    // Default node configuration values
    const DEFAULT_COOLDOWN_THRESHOLD: u32 = 1;
    const DEFAULT_COOLDOWN_SECS: u64 = 60;

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
                    cooldown_threshold: DEFAULT_COOLDOWN_THRESHOLD,
                    cooldown: Duration::from_secs(DEFAULT_COOLDOWN_SECS),
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

            match build_layered_http_provider(node_configs, pool_config.clone()) {
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
    use std::path::PathBuf;

    #[test]
    fn test_deserialize_bridges() {
        // Use CARGO_MANIFEST_DIR to get the project root, then navigate to config file
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = PathBuf::from(manifest_dir)
            .parent()
            .unwrap()
            .join("config/avalanche/bridges.json");
        let bridges = load_bridges_from_file(&path).unwrap();

        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].bridge_id, 2);
        assert_eq!(bridges[0].name, "Avalanche ICTT");
        assert_eq!(bridges[0].bridge_type, "avalanche_native");
        assert_eq!(bridges[0].contracts.len(), 2);
        assert_eq!(bridges[0].contracts[0].chain_id, 43114);
        assert_eq!(bridges[0].contracts[0].version, 1);
        assert_eq!(bridges[0].contracts[0].started_at_block, 42526120);
    }

    #[test]
    fn test_deserialize_chains() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = PathBuf::from(manifest_dir)
            .parent()
            .unwrap()
            .join("config/omnibridge/chains.json");
        let chains = load_chains_from_file(&path).unwrap();

        assert_eq!(chains.len(), 2);
        assert_eq!(chains[0].chain_id, 1);
        assert_eq!(chains[0].name, "Ethereum");
        // assert_eq!(chains[0].native_id, None);
        assert_eq!(chains[0].icon, "");
        assert!(!chains[0].rpcs.is_empty());

        assert_eq!(chains[1].chain_id, 100);
        assert_eq!(chains[1].name, "Gnosis");
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
        assert_eq!(config.bridge_type, "lockmint");
        assert!(config.enabled);
        assert_eq!(config.api_url, Some("https://api.example.com".to_string()));
        assert_eq!(config.ui_url, Some("https://ui.example.com".to_string()));
        assert_eq!(
            config.docs_url,
            Some("https://docs.example.com".to_string())
        );
        // indexer and contracts are lost in conversion (not stored in DB)
        assert_eq!(config.indexer, "");
        assert_eq!(config.contracts, vec![]);
    }

    #[test]
    fn test_bridge_contract_config_to_active_model() {
        let config = BridgeContractConfig {
            chain_id: 1,
            address: vec![0x12; 20],
            version: 1,
            started_at_block: 12345,
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
        };

        let config: BridgeContractConfig = model.into();

        assert_eq!(config.chain_id, 1);
        assert_eq!(config.address, vec![0x12; 20]);
        assert_eq!(config.version, 1);
        assert_eq!(config.started_at_block, 12345);
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
}
