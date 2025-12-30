use crate::{BridgeConfig, ChainConfig, Settings};
use alloy::{network::Ethereum, primitives::Address, providers::DynProvider};
use anyhow::{Context, Result};
use interchain_indexer_logic::{
    CrosschainIndexer, InterchainDatabase,
    indexer::avalanche::{AvalancheChainConfig, AvalancheIndexer, AvalancheIndexerConfig},
};
use std::{collections::HashMap, sync::Arc};

pub async fn spawn_configured_indexers(
    db: InterchainDatabase,
    bridges: &[BridgeConfig],
    chains: &[ChainConfig],
    chain_providers: &HashMap<u64, DynProvider<Ethereum>>,
    settings: &Settings,
) -> Result<Vec<Arc<dyn CrosschainIndexer>>> {
    let chain_lookup: HashMap<i64, ChainConfig> = chains
        .iter()
        .cloned()
        .map(|chain| (chain.chain_id, chain))
        .collect();

    let mut indexers: Vec<Arc<dyn CrosschainIndexer>> = Vec::new();

    for bridge in bridges {
        if !bridge.enabled {
            tracing::info!(bridge_id = bridge.bridge_id, "Skipping disabled bridge");
            continue;
        }

        match bridge.bridge_type.as_str() {
            "avalanche_native" => {
                let configs = build_avalanche_chain_configs(bridge, &chain_lookup, chain_providers);

                if configs.is_empty() {
                    tracing::warn!(
                        bridge_id = bridge.bridge_id,
                        "No viable chain configurations for Avalanche indexer, skipping"
                    );
                    continue;
                }

                let config = AvalancheIndexerConfig::new(
                    bridge.bridge_id,
                    configs,
                    &settings.avalanche_indexer,
                );
                let indexer =
                    AvalancheIndexer::new(Arc::new(db.clone()), config).with_context(|| {
                        format!(
                            "failed to spawn Avalanche indexer for bridge {}",
                            bridge.bridge_id
                        )
                    })?;
                let indexer: Arc<dyn CrosschainIndexer> = Arc::new(indexer);

                // Start indexer asynchronously.
                // NOTE: CrosschainIndexer::start is responsible for spawning internal tasks.
                // We intentionally don't keep JoinHandles here.
                // If start fails, we treat it as fatal for this indexer instance and skip it.
                indexer.start().await.with_context(|| {
                    format!(
                        "failed to start Avalanche indexer for bridge {}",
                        bridge.bridge_id
                    )
                })?;

                tracing::info!(bridge_id = bridge.bridge_id, "Started Avalanche indexer");
                indexers.push(indexer);
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
    Ok(indexers)
}

fn build_avalanche_chain_configs(
    bridge: &BridgeConfig,
    chain_lookup: &HashMap<i64, ChainConfig>,
    chain_providers: &HashMap<u64, DynProvider<Ethereum>>,
) -> Vec<AvalancheChainConfig> {
    let mut chain_configs = Vec::new();

    for contract in &bridge.contracts {
        let Some(_chain_config) = chain_lookup.get(&contract.chain_id) else {
            tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id = contract.chain_id,
                "Chain configuration missing for Avalanche indexer"
            );
            continue;
        };

        let Some(provider) = chain_providers.get(&(contract.chain_id as u64)) else {
            tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id = contract.chain_id,
                "No configured provider for chain"
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
            provider: provider.clone(),
            contract_address,
            start_block,
        });
    }

    chain_configs
}
