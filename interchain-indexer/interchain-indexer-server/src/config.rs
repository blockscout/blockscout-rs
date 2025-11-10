use alloy::primitives::Address;
use anyhow::{Context, Result};
use interchain_indexer_entity::{
    bridge_contracts, bridges, chains, sea_orm_active_enums::BridgeType,
};
use sea_orm::{ActiveValue, prelude::Json};
use serde::{Deserialize, Deserializer};
use serde_json;
use std::{collections::HashMap, path::Path, str::FromStr};

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

/// Getting type of bridge (BridgeType enum)
impl BridgeConfig {
    pub fn bridge_type_enum(&self) -> Option<BridgeType> {
        match self.bridge_type.as_str() {
            "lockmint" => Some(BridgeType::Lockmint),
            _ => None,
        }
    }
}

/// Convert BridgeConfig to bridges::ActiveModel for database operations
impl From<BridgeConfig> for bridges::ActiveModel {
    fn from(config: BridgeConfig) -> Self {
        let bridge_type = config.bridge_type_enum();
        bridges::ActiveModel {
            id: ActiveValue::Set(config.bridge_id),
            name: ActiveValue::Set(config.name),
            r#type: ActiveValue::Set(bridge_type),
            enabled: ActiveValue::Set(config.enabled),
            api_url: ActiveValue::Set(config.api_url),
            ui_url: ActiveValue::Set(config.ui_url),
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
            bridge_type: model
                .r#type
                .map(|t| match t {
                    BridgeType::Lockmint => "lockmint".to_string(),
                })
                .unwrap_or_default(),
            indexer: String::new(), // Not stored in database
            enabled: model.enabled,
            api_url: model.api_url,
            ui_url: model.ui_url,
            contracts: vec![], // Contracts are in a separate table
        }
    }
}

/// Convert BridgeContractConfig to bridge_contracts::ActiveModel for database operations
/// Note: `bridge_id` must be set separately as it's not part of BridgeContractConfig
impl BridgeContractConfig {
    pub fn to_active_model(&self, bridge_id: i32) -> bridge_contracts::ActiveModel {
        let abi_value = self.abi.as_ref().and_then(|abi_str| {
            serde_json::from_str::<serde_json::Value>(abi_str)
                .ok()
                .map(|v| Json::from(v))
        });

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
        chains::ActiveModel {
            id: ActiveValue::Set(config.chain_id),
            name: ActiveValue::Set(config.name),
            native_id: ActiveValue::Set(config.native_id),
            icon: ActiveValue::Set(if config.icon.is_empty() {
                None
            } else {
                Some(config.icon)
            }),
            ..Default::default()
        }
    }
}

/// Convert chains::Model to ChainConfig
/// Note: This conversion loses the `rpcs` field as it's not stored in the chains table
impl From<chains::Model> for ChainConfig {
    fn from(model: chains::Model) -> Self {
        ChainConfig {
            chain_id: model.id,
            name: model.name,
            native_id: model.native_id,
            icon: model.icon.unwrap_or_default(),
            rpcs: vec![], // RPCs are not stored in database
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ChainConfig {
    pub chain_id: i64,
    pub name: String,
    pub native_id: Option<String>,
    pub icon: String,
    pub rpcs: Vec<HashMap<String, RpcProviderConfig>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RpcProviderConfig {
    pub url: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: Option<ApiKeyConfig>,
}

fn default_enabled() -> bool {
    true
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
        assert_eq!(bridges[0].bridge_id, 1);
        assert_eq!(bridges[0].name, "Avalanche ICTT");
        assert_eq!(bridges[0].bridge_type, "lockmint");
        assert_eq!(bridges[0].contracts.len(), 2);
        assert_eq!(bridges[0].contracts[0].chain_id, 43114);
        assert_eq!(bridges[0].contracts[0].version, 1);
        assert_eq!(bridges[0].contracts[0].started_at_block, 42526120);
    }

    #[test]
    fn test_bridge_type_enum() {
        let bridge = BridgeConfig {
            bridge_id: 1,
            name: "Test".to_string(),
            bridge_type: "lockmint".to_string(),
            indexer: "Test".to_string(),
            enabled: true,
            api_url: None,
            ui_url: None,
            contracts: vec![],
        };

        assert_eq!(bridge.bridge_type_enum(), Some(BridgeType::Lockmint));
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
        assert_eq!(chains[0].native_id, None);
        assert_eq!(chains[0].icon, "");
        assert!(!chains[0].rpcs.is_empty());

        assert_eq!(chains[1].chain_id, 100);
        assert_eq!(chains[1].name, "Gnosis");
    }

    #[test]
    fn test_bridge_config_to_active_model() {
        let config = BridgeConfig {
            bridge_id: 1,
            name: "Test Bridge".to_string(),
            bridge_type: "lockmint".to_string(),
            indexer: "TestIndexer".to_string(),
            enabled: true,
            api_url: Some("https://api.example.com".to_string()),
            ui_url: Some("https://ui.example.com".to_string()),
            contracts: vec![],
        };

        let active_model: bridges::ActiveModel = config.clone().into();

        // Note: We can't easily check ActiveValue contents, but we can verify the conversion compiles
        // In a real scenario, you'd extract the values to verify
        assert!(matches!(active_model.id, ActiveValue::Set(1)));
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
            created_at: None,
            updated_at: None,
        };

        let config: BridgeConfig = model.into();

        assert_eq!(config.bridge_id, 1);
        assert_eq!(config.name, "Test Bridge");
        assert_eq!(config.bridge_type, "lockmint");
        assert_eq!(config.enabled, true);
        assert_eq!(config.api_url, Some("https://api.example.com".to_string()));
        assert_eq!(config.ui_url, Some("https://ui.example.com".to_string()));
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
            native_id: None,
            icon: "https://example.com/icon.png".to_string(),
            rpcs: vec![],
        };

        let active_model: chains::ActiveModel = config.clone().into();

        assert!(matches!(active_model.id, ActiveValue::Set(1)));
        assert!(matches!(active_model.name, ActiveValue::Set(ref name) if name == "Ethereum"));
        assert!(matches!(
            active_model.icon,
            ActiveValue::Set(Some(ref icon)) if icon == "https://example.com/icon.png"
        ));
    }

    #[test]
    fn test_chain_config_to_active_model_empty_icon() {
        let config = ChainConfig {
            chain_id: 1,
            name: "Ethereum".to_string(),
            native_id: None,
            icon: String::new(),
            rpcs: vec![],
        };

        let active_model: chains::ActiveModel = config.into();

        assert!(matches!(active_model.icon, ActiveValue::Set(None)));
    }

    #[test]
    fn test_model_to_chain_config() {
        use interchain_indexer_entity::chains;

        let model = chains::Model {
            id: 1,
            name: "Ethereum".to_string(),
            native_id: None,
            icon: Some("https://example.com/icon.png".to_string()),
            created_at: None,
            updated_at: None,
        };

        let config: ChainConfig = model.into();

        assert_eq!(config.chain_id, 1);
        assert_eq!(config.name, "Ethereum");
        assert_eq!(config.native_id, None);
        assert_eq!(config.icon, "https://example.com/icon.png");
        // rpcs are lost in conversion (not stored in DB)
        assert_eq!(config.rpcs, vec![]);
    }
}
