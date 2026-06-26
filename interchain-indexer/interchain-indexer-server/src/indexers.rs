// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{BridgeConfig, ChainConfig, Settings, config::IndexerType};
use alloy::{network::Ethereum, primitives::Address, providers::DynProvider};
use anyhow::{Context, Result};
use interchain_indexer_entity::sea_orm_active_enums::BridgeType;
use interchain_indexer_logic::{
    CrosschainIndexer, StatsService,
    indexer::{
        amb::{AmbChainConfig, AmbIndexer},
        avalanche::{AvalancheChainConfig, AvalancheIndexer},
    },
};
use std::{collections::HashMap, sync::Arc};

pub async fn spawn_configured_indexers(
    stats: Arc<StatsService>,
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
                        let indexer = AvalancheIndexer::new(
                            stats.clone(),
                            bridge.bridge_id,
                            configs,
                            bridge.home_chain_id,
                            bridge.process_unknown_chains,
                            &settings.avalanche_indexer,
                            &settings.buffer_settings,
                        )
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
            BridgeType::Amb => {
                let configs = build_amb_chain_configs(bridge, &chain_lookup, chain_providers);

                if configs.is_empty() {
                    tracing::warn!(
                        bridge_id = bridge.bridge_id,
                        "No viable chain configurations for AMB indexer, skipping"
                    );
                    continue;
                }

                let indexer: Arc<dyn CrosschainIndexer> = match bridge.indexer_type {
                    IndexerType::AMB => {
                        let indexer = AmbIndexer::new(
                            stats.clone(),
                            bridge.bridge_id,
                            configs,
                            &settings.amb_indexer,
                            &settings.buffer_settings,
                        )
                        .with_context(|| {
                            format!(
                                "failed to spawn AMB indexer for bridge {}",
                                bridge.bridge_id
                            )
                        })?;

                        Arc::new(indexer)
                    }
                    _ => {
                        tracing::error!(
                            bridge_id = bridge.bridge_id,
                            indexer_type =? bridge.indexer_type,
                            "Unsupported indexer type for AMB bridge"
                        );
                        continue;
                    }
                };

                if let Err(err) = indexer.start().await.with_context(|| {
                    format!(
                        "failed to start AMB indexer for bridge {}",
                        bridge.bridge_id
                    )
                }) {
                    tracing::error!(
                        bridge_id = bridge.bridge_id,
                        err = ?err,
                        "Failed to start AMB indexer"
                    );
                    continue;
                }

                tracing::info!(bridge_id = bridge.bridge_id, "Started AMB indexer");
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

fn build_amb_chain_configs(
    bridge: &BridgeConfig,
    chain_lookup: &HashMap<i64, ChainConfig>,
    chain_providers: &HashMap<i64, DynProvider<Ethereum>>,
) -> Vec<AmbChainConfig> {
    let mut by_chain: HashMap<i64, Vec<&crate::BridgeContractConfig>> = HashMap::new();
    for contract in &bridge.contracts {
        by_chain
            .entry(contract.chain_id)
            .or_default()
            .push(contract);
    }

    let mut chain_configs = Vec::new();
    for (chain_id, contracts) in by_chain {
        let Some(_chain_config) = chain_lookup.get(&chain_id) else {
            tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id,
                "Chain configuration missing for AMB indexer"
            );
            continue;
        };

        let Some(provider) = chain_providers.get(&chain_id) else {
            tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id,
                "No configured provider for chain"
            );
            continue;
        };

        let amb = contracts
            .iter()
            .copied()
            .find(|contract| contract.kind.as_deref() == Some("amb_proxy"));
        let mediator = contracts
            .iter()
            .copied()
            .find(|contract| contract.kind.as_deref() == Some("omnibridge_mediator"));

        let (Some(amb), Some(mediator)) = (amb, mediator) else {
            tracing::error!(
                bridge_id = bridge.bridge_id,
                chain_id,
                "AMB bridge requires amb_proxy and omnibridge_mediator contracts per chain"
            );
            continue;
        };

        let Some(amb_proxy_address) =
            parse_contract_address(bridge.bridge_id, chain_id, "amb_proxy", &amb.address)
        else {
            continue;
        };
        let Some(mediator_address) = parse_contract_address(
            bridge.bridge_id,
            chain_id,
            "omnibridge_mediator",
            &mediator.address,
        ) else {
            continue;
        };

        let amb_abi = parse_contract_abi(bridge.bridge_id, chain_id, "amb_proxy", amb.abi.as_ref());
        let mediator_abi = parse_contract_abi(
            bridge.bridge_id,
            chain_id,
            "omnibridge_mediator",
            mediator.abi.as_ref(),
        );

        chain_configs.push(AmbChainConfig {
            chain_id,
            provider: provider.clone(),
            amb_proxy_address,
            mediator_address,
            start_block: amb.started_at_block,
            amb_version: amb.version,
            mediator_version: mediator.version,
            amb_abi,
            mediator_abi,
        });
    }

    chain_configs
}

fn parse_contract_address(
    bridge_id: i32,
    chain_id: i64,
    kind: &str,
    address: &[u8],
) -> Option<Address> {
    let Ok(address_bytes): Result<[u8; 20], _> = address.try_into() else {
        tracing::error!(
            bridge_id,
            chain_id,
            kind,
            "Bridge contract address must be 20 bytes"
        );
        return None;
    };
    Some(Address::from(address_bytes))
}

fn parse_contract_abi(
    bridge_id: i32,
    chain_id: i64,
    kind: &str,
    abi: Option<&String>,
) -> Option<serde_json::Value> {
    match abi {
        Some(abi) => match serde_json::from_str(abi) {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::error!(
                    bridge_id,
                    chain_id,
                    kind,
                    err = ?err,
                    "Invalid ABI JSON in AMB contract config"
                );
                None
            }
        },
        None => None,
    }
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
