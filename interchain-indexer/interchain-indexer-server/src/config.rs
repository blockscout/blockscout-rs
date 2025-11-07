use alloy::primitives::Address;
use anyhow::{Context, Result};
use interchain_indexer_entity::sea_orm_active_enums::BridgeType;
use serde::{Deserialize, Deserializer};
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
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct BridgesJson {
    bridges: Vec<BridgeConfig>,
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

/// Load and deserialize bridges from a JSON file
pub fn load_bridges_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<BridgeConfig>> {
    let content = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("Failed to read bridges config file: {:?}", path.as_ref()))?;
    
    let bridges_json: BridgesJson = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse bridges config JSON: {:?}", path.as_ref()))?;
    
    Ok(bridges_json.bridges)
}

/// Convert BridgeConfig to BridgeType enum
impl BridgeConfig {
    pub fn bridge_type_enum(&self) -> Option<BridgeType> {
        match self.bridge_type.as_str() {
            "lockmint" => Some(BridgeType::Lockmint),
            _ => None,
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
}

