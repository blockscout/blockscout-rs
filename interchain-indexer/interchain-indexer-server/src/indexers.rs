use crate::{BridgeConfig, ChainConfig, Settings, config::IndexerType};
use alloy::{network::Ethereum, primitives::Address, providers::DynProvider};
use anyhow::{Context, Result};
use interchain_indexer_entity::sea_orm_active_enums::BridgeType;
use interchain_indexer_logic::{
    CrosschainIndexer, InterchainDatabase,
    indexer::avalanche::{AvalancheChainConfig, AvalancheIndexer, AvalancheIndexerConfig},
};
use std::{collections::HashMap, sync::Arc};

pub async fn spawn_configured_indexers(
    db: InterchainDatabase,
    bridges: &[BridgeConfig],
    chains: &[ChainConfig],
    chain_providers: &HashMap<i64, DynProvider<Ethereum>>,
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

        match bridge.bridge_type {
            BridgeType::AvalancheNative => {
                let configs = build_avalanche_chain_configs(bridge, &chain_lookup, chain_providers);

                if configs.is_empty() {
                    tracing::warn!(
                        bridge_id = bridge.bridge_id,
                        "No viable chain configurations for Avalanche indexer, skipping"
                    );
                    continue;
                }

                let indexer: Arc<dyn CrosschainIndexer> = match bridge.indexer_type {
                    IndexerType::IcmIctt => {
                        let config = AvalancheIndexerConfig::new(
                            bridge.bridge_id,
                            configs,
                            &settings.avalanche_indexer,
                        )
                        .with_buffer_settings(settings.buffer_settings.clone());
                        let indexer = AvalancheIndexer::new(Arc::new(db.clone()), config)
                            .with_context(|| {
                                format!(
                                    "failed to spawn Avalanche indexer for bridge {}",
                                    bridge.bridge_id
                                )
                            })?;

                        Arc::new(indexer)
                    }
                    _ => {
                        tracing::error!(
                            bridge_id = bridge.bridge_id,
                            indexer_type =? bridge.indexer_type,
                            "Unsupported indexer type for Avalanche indexer"
                        );
                        continue;
                    }
                };

                // Start indexer asynchronously.
                // NOTE: CrosschainIndexer::start is responsible for spawning internal tasks.
                // We intentionally don't keep JoinHandles here.
                // If start fails, we treat it as fatal for this indexer instance and skip it.
                if let Err(err) = indexer.start().await.with_context(|| {
                    format!(
                        "failed to start Avalanche indexer for bridge {}",
                        bridge.bridge_id
                    )
                }) {
                    tracing::error!(
                        bridge_id = bridge.bridge_id,
                        err = ?err,
                        "Failed to start Avalanche indexer"
                    );
                    continue;
                }

                tracing::info!(bridge_id = bridge.bridge_id, "Started Avalanche indexer");
                indexers.push(indexer);
            }
            _ => {
                tracing::warn!(
                    bridge_id = bridge.bridge_id,
                    bridge_type =? bridge.bridge_type,
                    "No indexer has been implemented for this bridge type yet."
                );
            }
        }
    }
    Ok(indexers)
}

fn build_avalanche_chain_configs(
    bridge: &BridgeConfig,
    chain_lookup: &HashMap<i64, ChainConfig>,
    chain_providers: &HashMap<i64, DynProvider<Ethereum>>,
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

        let Some(provider) = chain_providers.get(&(contract.chain_id)) else {
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

        chain_configs.push(AvalancheChainConfig {
            chain_id: contract.chain_id,
            start_block: contract.started_at_block,
            provider: provider.clone(),
            contract_address,
        });
    }

    chain_configs
}
