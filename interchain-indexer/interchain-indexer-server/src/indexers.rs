use crate::{BridgeConfig, ChainConfig};
use alloy::primitives::Address;
use anyhow::{Context, Result};
use interchain_indexer_logic::{
    InterchainDatabase,
    indexers::avalanche::{AvalancheChainConfig, AvalancheIndexerConfig, spawn_indexer},
};
use std::collections::HashMap;
use tokio::task::JoinHandle;

pub fn spawn_configured_indexers(
    db: InterchainDatabase,
    bridges: &[BridgeConfig],
    chains: &[ChainConfig],
) -> Result<Vec<JoinHandle<()>>> {
    let chain_lookup: HashMap<i64, ChainConfig> = chains
        .iter()
        .cloned()
        .map(|chain| (chain.chain_id, chain))
        .collect();

    let mut handles = Vec::new();

    for bridge in bridges {
        if !bridge.enabled {
            tracing::info!(bridge_id = bridge.bridge_id, "Skipping disabled bridge");
            continue;
        }

        match bridge.bridge_type.as_str() {
            "avalanche_native" => {
                let configs = build_avalanche_chain_configs(bridge, &chain_lookup);

                if configs.is_empty() {
                    tracing::warn!(
                        bridge_id = bridge.bridge_id,
                        "No viable chain configurations for Avalanche indexer, skipping"
                    );
                    continue;
                }

                let config = AvalancheIndexerConfig::new(bridge.bridge_id, configs);
                let handle = spawn_indexer(db.clone(), config).with_context(|| {
                    format!(
                        "failed to spawn Avalanche indexer for bridge {}",
                        bridge.bridge_id
                    )
                })?;
                tracing::info!(bridge_id = bridge.bridge_id, "Spawned Avalanche indexer");
                handles.push(handle);
            }
            other => {
                tracing::debug!(
                    bridge_id = bridge.bridge_id,
                    indexer = other,
                    "No indexer implementation configured for bridge"
                );
            }
        }
    }

    Ok(handles)
}

fn build_avalanche_chain_configs(
    bridge: &BridgeConfig,
    chain_lookup: &HashMap<i64, ChainConfig>,
) -> Vec<AvalancheChainConfig> {
    let mut chain_configs = Vec::new();

    for contract in &bridge.contracts {
        let Some(chain_config) = chain_lookup.get(&contract.chain_id) else {
            tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id = contract.chain_id,
                "Chain configuration missing for Avalanche indexer"
            );
            continue;
        };

        let Some(rpc_url) = select_primary_rpc(chain_config) else {
            tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id = contract.chain_id,
                "No enabled RPC endpoints for chain"
            );
            continue;
        };

        let Ok(address_bytes): Result<[u8; 20], _> = contract.address.clone().try_into() else {
            tracing::error!(
                bridge_id = bridge.bridge_id,
                chain_id = contract.chain_id,
                "Bridge contract address must be 20 bytes"
            );
            continue;
        };
        let contract_address = Address::from(address_bytes);
        let start_block = contract.started_at_block.max(0) as u64;

        chain_configs.push(AvalancheChainConfig {
            chain_id: contract.chain_id,
            rpc_url,
            contract_address,
            start_block,
        });
    }

    chain_configs
}

fn select_primary_rpc(chain: &ChainConfig) -> Option<String> {
    chain
        .rpcs
        .iter()
        .flat_map(|provider_map| provider_map.values())
        .find(|cfg| cfg.enabled)
        .map(|cfg| cfg.url.clone())
}
