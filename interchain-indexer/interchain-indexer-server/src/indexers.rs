// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{BridgeConfig, BridgeContractConfig, ChainConfig, Settings, config::IndexerType};
use alloy::{network::Ethereum, primitives::Address, providers::DynProvider};
use anyhow::{Context, Result};
use interchain_indexer_entity::{bridge_contracts, sea_orm_active_enums::BridgeType};
use interchain_indexer_logic::{
    CrosschainIndexer, InterchainDatabase, StatsService,
    indexer::{
        amb::{AmbChainConfig, AmbIndexer},
        avalanche::{AvalancheChainConfig, AvalancheIndexer},
    },
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

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
                            bridge.reconstruct_incoming_ictt_transfers,
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
            // Mirrored by `scan_floor_for_pair` below (used for progress-API
            // enumeration and startup floor reconciliation). Keep both in
            // sync: drift here silently mis-reports the denominator for
            // every AMB pair.
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

// --- Indexing-progress API support ---
//
// `scan_floor_for_pair`, `enumerate_indexing_targets` and
// `reconcile_catchup_floors` are the config-driven surface behind
// `GetIndexingProgress`. See `.memory-bank/research/message-lifecycle.md` §2
// for the cursor semantics these functions rely on.

/// Groups a bridge's configured contracts by `chain_id`, so each pair is
/// considered once regardless of how many contracts it declares (AMB
/// declares two per chain).
fn group_contracts_by_chain(
    contracts: &[BridgeContractConfig],
) -> HashMap<i64, Vec<&BridgeContractConfig>> {
    let mut by_chain: HashMap<i64, Vec<&BridgeContractConfig>> = HashMap::new();
    for contract in contracts {
        by_chain
            .entry(contract.chain_id)
            .or_default()
            .push(contract);
    }
    by_chain
}

/// The block a `(bridge, chain)` pair's *scan* starts from — i.e. the value handed to
/// `LogStream.genesis_block`.
///
/// For `BridgeType::Amb` that is the `kind == "amb_proxy"` contract's `started_at_block`,
/// mirroring `build_amb_chain_configs` (see the `start_block:` field around indexers.rs:247).
/// The `omnibridge_mediator` value is deliberately NOT used: in
/// `config/omnibridge/bridges.json` it is 7.4M blocks lower on chain 1, so `min` across
/// contracts would badly understate the denominator.
///
/// For every other bridge type, `min` across the chain's configured contracts (Avalanche
/// declares exactly one contract per chain, so `min` is that contract's value).
/// An AMB chain with no `amb_proxy` contract falls back to `min`.
pub(crate) fn scan_floor_for_pair(
    bridge_type: BridgeType,
    contracts: &[&BridgeContractConfig],
) -> Option<u64> {
    if bridge_type == BridgeType::Amb
        && let Some(amb) = contracts
            .iter()
            .find(|contract| contract.kind.as_deref() == Some("amb_proxy"))
    {
        return Some(amb.started_at_block);
    }

    contracts
        .iter()
        .map(|contract| contract.started_at_block)
        .min()
}

/// One `(bridge, chain)` the service is configured to index, with the block the scan
/// starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexingTarget {
    pub bridge_id: i32,
    pub chain_id: i64,
    pub start_block: u64,
}

/// Enumerates every `(bridge, chain)` pair the service is configured to index, from the
/// in-memory bridges config — never from `bridge_contracts`, which is under-populated
/// during startup and permanently over-populated after a chain is dropped from a bridge
/// (`.memory-bank/gotchas.md` → "`bridge_contracts` Is Only A Diagnostic Proxy For Runtime
/// Membership").
///
/// Disabled bridges are excluded (they have no indexer by design and would sit at 0%
/// forever). Everything else is included even when no indexer could actually be built —
/// a missing `chains.json` entry, a missing provider, an unsupported `indexer_type`, or an
/// AMB chain lacking its `amb_proxy`/`omnibridge_mediator` pair — because a pair whose
/// indexer failed to start is exactly the case this enumeration exists to surface at 0%
/// instead of silently omitting it.
///
/// Sorted by `(bridge_id, chain_id)`.
pub fn enumerate_indexing_targets(bridges: &[BridgeConfig]) -> Vec<IndexingTarget> {
    let mut targets = Vec::new();

    for bridge in bridges {
        if !bridge.enabled {
            continue;
        }

        for (chain_id, contracts) in group_contracts_by_chain(&bridge.contracts) {
            let start_block = match scan_floor_for_pair(bridge.bridge_type.clone(), &contracts) {
                Some(start_block) => start_block,
                None => {
                    tracing::warn!(
                        bridge_id = bridge.bridge_id,
                        chain_id,
                        "no scan floor could be derived for this configured pair; reporting start_block = 0"
                    );
                    0
                }
            };

            targets.push(IndexingTarget {
                bridge_id: bridge.bridge_id,
                chain_id,
                start_block,
            });
        }
    }

    targets.sort_by_key(|target| (target.bridge_id, target.chain_id));
    targets
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FloorReconciliation {
    /// The configured floor was lowered: assign the new, lower value.
    Lower(u64),
    /// Nothing to do — progress is preserved, or the seed's `GREATEST` handles it.
    NoAction,
}

/// `prev` is the previous run's configured scan floor, read from `bridge_contracts`
/// before `upsert_bridge_contracts` overwrites it. `None` means "previous unknown" —
/// treating it as `0` would classify every legacy row as a raise and silently defeat
/// the whole mechanism.
pub(crate) fn decide_floor_reconciliation(prev: Option<u64>, new: u64) -> FloorReconciliation {
    match prev {
        Some(prev) if prev > new => FloorReconciliation::Lower(new),
        // `prev == new` (progress preserved), `prev < new` (the seed's `GREATEST`
        // raises it), and `prev == None` (unknown, not a raise) all take no action.
        _ => FloorReconciliation::NoAction,
    }
}

/// Derives the previous run's scan floor by applying the exact same selection rule as
/// `scan_floor_for_pair`, but over `bridge_contracts` rows instead of config values, so
/// `prev` and `new` can never disagree because of a rule mismatch (e.g. editing the AMB
/// mediator's `started_at_block` must never look like a change in scanning).
///
/// Identity-scoped lookup: only the identities currently in `contracts` (i.e. currently in
/// config) are looked up by `(chain_id, address, version)`, `bridge_contracts`' unique key —
/// a stale stored row for an address no longer in config can never influence the result.
///
/// Returns `None` ("previous unknown") whenever a *relevant* contract (the `amb_proxy` for
/// AMB, every configured contract for the min rule) has no stored row yet, or a stored row
/// with `started_at_block IS NULL`. `bridge_contracts.started_at_block` is deliberately read
/// directly (`Option<i64>`), never through
/// `bridge_contracts::Model::validated_started_at_block()`, which maps `None` to `0` and
/// would read a genuinely unknown previous value as a raise.
fn derive_prev_floor(
    bridge_id: i32,
    chain_id: i64,
    bridge_type: BridgeType,
    contracts: &[&BridgeContractConfig],
    stored: &[bridge_contracts::Model],
) -> Option<u64> {
    let lookup = |contract: &BridgeContractConfig| -> Option<i64> {
        let stored_row = stored.iter().find(|row| {
            row.chain_id == contract.chain_id
                && row.address == contract.address
                && row.version == contract.version
        });

        match stored_row {
            Some(row) if row.started_at_block.is_none() => {
                tracing::warn!(
                    bridge_id,
                    chain_id,
                    kind = ?contract.kind,
                    "stored bridge_contracts row has a NULL started_at_block; treating previous scan floor as unknown"
                );
                None
            }
            Some(row) => row.started_at_block,
            None => {
                tracing::debug!(
                    bridge_id,
                    chain_id,
                    kind = ?contract.kind,
                    "no stored bridge_contracts row yet for this configured contract; treating previous scan floor as unknown"
                );
                None
            }
        }
    };

    if bridge_type == BridgeType::Amb
        && let Some(amb) = contracts
            .iter()
            .find(|contract| contract.kind.as_deref() == Some("amb_proxy"))
    {
        return lookup(amb).and_then(|value| u64::try_from(value).ok());
    }

    // Fallback -- mirrors `scan_floor_for_pair`: non-AMB bridges, and an AMB
    // chain with no `amb_proxy` contract, both use `min` across the pair's
    // configured contracts' stored values.
    let mut floor: Option<u64> = None;
    for contract in contracts {
        let value = u64::try_from(lookup(contract)?).ok()?;
        floor = Some(floor.map_or(value, |current| current.min(value)));
    }
    floor
}

/// Detects a `started_at_block` that was lowered since the previous run and lowers the
/// stored `catchup_min_cursor` to match.
///
/// MUST be called BEFORE `upsert_bridge_contracts` — see `server.rs`. `bridge_contracts`
/// only still holds the *previous* run's `started_at_block` in the window before that
/// call's `ON CONFLICT` overwrites it; reordering this call breaks detection with no test
/// failure unless the ordering itself is asserted.
///
/// Reconciles **every** bridge, including disabled ones. This is a deliberate deviation
/// from the original design, which skipped disabled bridges here on the assumption that
/// `upsert_bridge_contracts` would too. It does
/// not: `server.rs` builds that payload from *all* configured bridges with no `enabled`
/// filter, so a disabled bridge's stored `started_at_block` is refreshed regardless. Skipping
/// it here would consume the previous value with nothing having looked at it, permanently
/// defeating the reset for the sequence disable → lower `started_at_block` → restart →
/// re-enable → restart. Lowering a stored floor for a disabled bridge is harmless — nothing
/// indexes it, and `enumerate_indexing_targets` still excludes disabled bridges from the
/// progress endpoint's output.
///
/// Never panics or propagates an error: every failure is logged and skipped, and the
/// bridge's id is returned so the caller can withhold that bridge's `bridge_contracts` rows
/// from this startup's `upsert_bridge_contracts` payload (see `bridges_pending_contracts_upsert`).
/// Refreshing an unreconciled bridge's rows anyway would overwrite the only place the
/// previous `started_at_block` survives, permanently losing the evidence needed to detect a
/// lowered floor — no later restart could recover it, since `prev == new` forever after.
/// Excluding it instead makes the failure retryable: the next startup sees the still-stale
/// `bridge_contracts` row and can complete the reconciliation. This does **not** change the
/// severity of the failure itself — it is still a `warn`, not fatal — it only ensures the
/// failure does not become permanent.
///
/// This is deliberately a `warn`, not fatal — safe only while catch-up is one-directional,
/// because the downward scan takes its floor from the config value handed to
/// `LogStream.genesis_block` and ignores `catchup_min_cursor` entirely, so a failed reset
/// costs a wrong reading on the progress endpoint and loses no data. **Whoever makes
/// `catchup_min_cursor` a real scan boundary must first make this write's failure fatal for
/// the pair** — under a bidirectional indexer, the identical failure would silently drop the
/// newly opened range instead of merely mis-reporting it.
///
/// Returns the set of bridge ids that could not be fully reconciled this startup (a failed
/// `get_bridge_contracts` read, or a failed `lower_catchup_floor` write for any of the
/// bridge's pairs).
pub async fn reconcile_catchup_floors(
    db: &InterchainDatabase,
    bridges: &[BridgeConfig],
) -> HashSet<i32> {
    let mut unreconciled_bridges = HashSet::new();

    for bridge in bridges {
        let stored_contracts = match db.get_bridge_contracts(bridge.bridge_id).await {
            Ok(contracts) => contracts,
            Err(err) => {
                tracing::warn!(
                    err = ?err,
                    bridge_id = bridge.bridge_id,
                    "failed to read stored bridge contracts for catchup floor reconciliation; skipping bridge"
                );
                unreconciled_bridges.insert(bridge.bridge_id);
                continue;
            }
        };

        for (chain_id, contracts) in group_contracts_by_chain(&bridge.contracts) {
            let Some(new_floor) = scan_floor_for_pair(bridge.bridge_type.clone(), &contracts)
            else {
                tracing::warn!(
                    bridge_id = bridge.bridge_id,
                    chain_id,
                    "no scan floor could be derived for this configured pair; skipping catchup floor reconciliation"
                );
                continue;
            };

            let prev_floor = derive_prev_floor(
                bridge.bridge_id,
                chain_id,
                bridge.bridge_type.clone(),
                &contracts,
                &stored_contracts,
            );

            if let FloorReconciliation::Lower(new_floor) =
                decide_floor_reconciliation(prev_floor, new_floor)
            {
                match db
                    .lower_catchup_floor(bridge.bridge_id, chain_id, new_floor)
                    .await
                {
                    Ok(rows_affected) => tracing::info!(
                        bridge_id = bridge.bridge_id,
                        chain_id,
                        prev = ?prev_floor,
                        new = new_floor,
                        rows_affected,
                        "lowered stored catchup floor to match a lowered configured started_at_block"
                    ),
                    Err(err) => {
                        tracing::warn!(
                            err = ?err,
                            bridge_id = bridge.bridge_id,
                            chain_id,
                            prev = ?prev_floor,
                            new = new_floor,
                            "failed to lower catchup floor; startup continues, progress reporting may be stale for this pair"
                        );
                        unreconciled_bridges.insert(bridge.bridge_id);
                    }
                }
            }
        }
    }

    unreconciled_bridges
}

/// Bridges whose `bridge_contracts` rows should be refreshed by this startup's
/// `upsert_bridge_contracts` call: every configured bridge except those
/// `reconcile_catchup_floors` could not fully reconcile.
///
/// This is the fix for the permanence gap described on `reconcile_catchup_floors`: the
/// previous run's `started_at_block` survives only in `bridge_contracts`, and
/// `upsert_bridge_contracts`'s `ON CONFLICT` overwrites it a few statements later. Refreshing
/// an unreconciled bridge's rows anyway destroys that evidence on the very startup that
/// failed to act on it. Excluding the bridge instead defers the refresh — and with it, the
/// reconciliation — to the next startup, where it can be retried.
pub(crate) fn bridges_pending_contracts_upsert<'a>(
    bridges: &'a [BridgeConfig],
    unreconciled_bridges: &HashSet<i32>,
) -> Vec<&'a BridgeConfig> {
    // Partitioned rather than filtered with a logging side effect in the
    // predicate: the warning must fire exactly once per skipped bridge, and a
    // predicate that logs only does so while the consumer happens to drain the
    // iterator fully. Returning a lazy iterator, or a caller reaching for
    // `any()`/`take()`, would silently drop or duplicate the warnings — and
    // this warning is the only signal that a refresh was deferred.
    let (pending, skipped): (Vec<_>, Vec<_>) = bridges
        .iter()
        .partition(|bridge| !unreconciled_bridges.contains(&bridge.bridge_id));

    for bridge in skipped {
        tracing::warn!(
            bridge_id = bridge.bridge_id,
            "skipping bridge_contracts refresh this startup: catchup floor \
             reconciliation could not be completed for this bridge, so overwriting \
             started_at_block now would permanently lose the evidence needed to \
             detect a lowered floor; deferring the refresh (and the reconciliation) \
             to the next startup"
        );
    }

    pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn omnibridge_config_path() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .expect("manifest dir has a parent")
            .join("config/omnibridge/bridges.json")
    }

    fn contract(
        chain_id: i64,
        kind: Option<&str>,
        started_at_block: u64,
        version: i16,
    ) -> BridgeContractConfig {
        BridgeContractConfig {
            chain_id,
            address: vec![0xAB; 20],
            version,
            started_at_block,
            kind: kind.map(str::to_string),
            abi: None,
        }
    }

    fn bridge(
        bridge_id: i32,
        bridge_type: BridgeType,
        enabled: bool,
        contracts: Vec<BridgeContractConfig>,
    ) -> BridgeConfig {
        BridgeConfig {
            bridge_id,
            name: format!("bridge-{bridge_id}"),
            bridge_type,
            indexer_type: IndexerType::Unknown,
            enabled,
            api_url: None,
            ui_url: None,
            docs_url: None,
            process_unknown_chains: false,
            home_chain_id: None,
            reconstruct_incoming_ictt_transfers: true,
            contracts,
        }
    }

    // --- enumerate_indexing_targets / scan_floor_for_pair ---

    #[test]
    fn test_enumerate_indexing_targets_amb_config_pins_amb_proxy_floors_not_mediator() {
        // Regression test for drift against `build_amb_chain_configs`'s
        // `start_block: amb.started_at_block` (this file, near the AMB chain
        // config push): the progress denominator must come from the exact
        // same contract that the running indexer actually starts from.
        let bridges = crate::load_bridges_from_file(omnibridge_config_path()).unwrap();
        let targets = enumerate_indexing_targets(&bridges);

        let chain_1 = targets
            .iter()
            .find(|t| t.bridge_id == 1 && t.chain_id == 1)
            .expect("chain 1 must be enumerated");
        let chain_100 = targets
            .iter()
            .find(|t| t.bridge_id == 1 && t.chain_id == 100)
            .expect("chain 100 must be enumerated");

        assert_eq!(chain_1.start_block, 20812229);
        assert_eq!(chain_100.start_block, 36145833);
        assert_ne!(
            chain_1.start_block, 13424376,
            "must not report the omnibridge_mediator's started_at_block"
        );
        assert_ne!(
            chain_100.start_block, 18588922,
            "must not report the omnibridge_mediator's started_at_block"
        );
    }

    #[test]
    fn test_enumerate_indexing_targets_one_target_per_pair_for_chain_with_several_contracts() {
        let bridges = vec![bridge(
            1,
            BridgeType::Amb,
            true,
            vec![
                contract(1, Some("amb_proxy"), 100, 1),
                contract(1, Some("omnibridge_mediator"), 50, 1),
            ],
        )];

        let targets = enumerate_indexing_targets(&bridges);
        assert_eq!(
            targets,
            vec![IndexingTarget {
                bridge_id: 1,
                chain_id: 1,
                start_block: 100
            }]
        );
    }

    #[test]
    fn test_enumerate_indexing_targets_disabled_bridge_contributes_nothing() {
        let bridges = vec![bridge(
            1,
            BridgeType::AvalancheNative,
            false,
            vec![contract(1, None, 100, 1)],
        )];
        assert_eq!(enumerate_indexing_targets(&bridges), Vec::new());
    }

    #[test]
    fn test_enumerate_indexing_targets_chain_without_chains_json_entry_is_still_enumerated() {
        // enumerate_indexing_targets never consults chains.json or providers,
        // only the bridges config -- this is the entire point of
        // config-driven enumeration.
        let bridges = vec![bridge(
            1,
            BridgeType::AvalancheNative,
            true,
            vec![contract(999_999, None, 42, 1)],
        )];
        let targets = enumerate_indexing_targets(&bridges);
        assert_eq!(
            targets,
            vec![IndexingTarget {
                bridge_id: 1,
                chain_id: 999_999,
                start_block: 42
            }]
        );
    }

    #[test]
    fn test_enumerate_indexing_targets_output_sorted_by_bridge_and_chain() {
        let bridges = vec![
            bridge(
                2,
                BridgeType::AvalancheNative,
                true,
                vec![contract(5, None, 1, 1)],
            ),
            bridge(
                1,
                BridgeType::AvalancheNative,
                true,
                vec![contract(200, None, 1, 1), contract(100, None, 1, 1)],
            ),
        ];

        let targets = enumerate_indexing_targets(&bridges);
        let pairs: Vec<(i32, i64)> = targets.iter().map(|t| (t.bridge_id, t.chain_id)).collect();
        assert_eq!(pairs, vec![(1, 100), (1, 200), (2, 5)]);
    }

    // --- decide_floor_reconciliation ---

    #[test]
    fn test_decide_floor_reconciliation_prev_greater_than_new_lowers() {
        assert_eq!(
            decide_floor_reconciliation(Some(1000), 500),
            FloorReconciliation::Lower(500)
        );
    }

    #[test]
    fn test_decide_floor_reconciliation_prev_equal_new_no_action() {
        assert_eq!(
            decide_floor_reconciliation(Some(500), 500),
            FloorReconciliation::NoAction
        );
    }

    #[test]
    fn test_decide_floor_reconciliation_prev_less_than_new_no_action() {
        assert_eq!(
            decide_floor_reconciliation(Some(100), 500),
            FloorReconciliation::NoAction
        );
    }

    #[test]
    fn test_decide_floor_reconciliation_prev_none_no_action() {
        assert_eq!(
            decide_floor_reconciliation(None, 500),
            FloorReconciliation::NoAction
        );
    }

    // --- reconcile_catchup_floors (DB-backed) ---

    use blockscout_service_launcher::test_database::TestDbGuard;
    use interchain_indexer_entity::{bridges, chains};
    use interchain_indexer_logic::indexer::progress::{CatchupProgress, CheckpointCursors};
    use sea_orm::ActiveValue::Set;

    async fn init_db(name: &str) -> TestDbGuard {
        TestDbGuard::new::<migration::Migrator>(name).await
    }

    fn contract_with_address(
        chain_id: i64,
        kind: Option<&str>,
        started_at_block: u64,
        version: i16,
        address_byte: u8,
    ) -> BridgeContractConfig {
        BridgeContractConfig {
            chain_id,
            address: vec![address_byte; 20],
            version,
            started_at_block,
            kind: kind.map(str::to_string),
            abi: None,
        }
    }

    async fn seed_bridge_and_chain(db: &InterchainDatabase, bridge_id: i32, chain_id: i64) {
        db.upsert_bridges(vec![bridges::ActiveModel {
            id: Set(bridge_id),
            name: Set(format!("bridge-{bridge_id}")),
            enabled: Set(true),
            ..Default::default()
        }])
        .await
        .unwrap();
        db.upsert_chains(vec![chains::ActiveModel {
            id: Set(chain_id),
            name: Set(format!("chain-{chain_id}")),
            ..Default::default()
        }])
        .await
        .unwrap();
    }

    /// Stores a `bridge_contracts` row directly, bypassing
    /// `BridgeContractConfig::to_active_model` so `started_at_block` can be
    /// `None` -- exercising the "previous unknown" fixture that
    /// `BridgeContractConfig` (a required `u64` in config) cannot represent.
    async fn store_bridge_contract(
        db: &InterchainDatabase,
        bridge_id: i32,
        chain_id: i64,
        address: &[u8],
        version: i16,
        kind: Option<&str>,
        started_at_block: Option<i64>,
    ) {
        db.upsert_bridge_contracts(vec![bridge_contracts::ActiveModel {
            bridge_id: Set(bridge_id),
            chain_id: Set(chain_id),
            address: Set(address.to_vec()),
            version: Set(version),
            started_at_block: Set(started_at_block),
            kind: Set(kind.map(str::to_string)),
            ..Default::default()
        }])
        .await
        .unwrap();
    }

    async fn checkpoint_floor(db: &InterchainDatabase, bridge_id: i32, chain_id: i64) -> i64 {
        db.get_checkpoint(bridge_id as u64, chain_id as u64)
            .await
            .unwrap()
            .expect("checkpoint row must exist")
            .catchup_min_cursor
    }

    /// Encodes the exact bug the reconciliation exists to fix (ADR-005): a
    /// lowered `started_at_block` left un-reconciled makes a fully-scanned
    /// stale floor read as "complete" for the *widened* range, forever.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_lowers_stored_floor_and_fixes_completion() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_lowers_floor_fixes_completion").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        // Previous run: started_at_block = 1000, stored in bridge_contracts,
        // catch-up already completed (X = 999).
        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xAA; 20], 1, None, Some(1000)).await;
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 1000, 999, 2000)
            .await
            .unwrap();

        let new_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];

        // Without reconciliation, the stale floor (1000) against the new,
        // lower start_block (500) reports the widened range as complete --
        // exactly the bug this feature exists to fix.
        let stale_progress = CatchupProgress::compute(
            500,
            Some(CheckpointCursors {
                catchup_min_cursor: checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await as u64,
                catchup_max_cursor: 999,
                realtime_cursor: 2000,
            }),
        );
        assert!(stale_progress.scan_complete);
        assert_eq!(stale_progress.blocks_remaining, 0);

        reconcile_catchup_floors(&db, &new_config).await;

        let checkpoint = db
            .get_checkpoint(BRIDGE_ID as u64, CHAIN_ID as u64)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.catchup_min_cursor, 500);
        assert_eq!(
            checkpoint.catchup_max_cursor, 999,
            "reconciliation must not touch catchup_max_cursor"
        );

        let fixed_progress = CatchupProgress::compute(
            500,
            Some(CheckpointCursors {
                catchup_min_cursor: checkpoint.validated_catchup_min_cursor(),
                catchup_max_cursor: checkpoint.validated_catchup_cursor(),
                realtime_cursor: checkpoint.validated_realtime_cursor(),
            }),
        );
        assert!(!fixed_progress.scan_complete);
        assert_eq!(fixed_progress.blocks_remaining, 500);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_unchanged_config_value_leaves_advanced_floor_untouched() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_unchanged_leaves_floor").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xAA; 20], 1, None, Some(800)).await;
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 800, 999, 2000)
            .await
            .unwrap();

        let unchanged_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 800, 1, 0xAA)],
        )];

        reconcile_catchup_floors(&db, &unchanged_config).await;

        assert_eq!(checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await, 800);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_raised_config_value_is_left_to_the_seed() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_raised_left_to_seed").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xAA; 20], 1, None, Some(500)).await;
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 500, 999, 2000)
            .await
            .unwrap();

        let raised_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 800, 1, 0xAA)],
        )];

        reconcile_catchup_floors(&db, &raised_config).await;
        assert_eq!(
            checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await,
            500,
            "reconciliation only lowers; a raise is the seed's job"
        );

        // The seed call the stream builder makes at startup is what actually
        // raises it, via its own GREATEST conflict rule.
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 800, 999, 2000)
            .await
            .unwrap();
        assert_eq!(checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await, 800);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_null_previous_started_at_block_performs_no_write() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_null_previous_no_write").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        // Stored row exists but `started_at_block IS NULL`.
        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xAA; 20], 1, None, None).await;

        let config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];

        reconcile_catchup_floors(&db, &config).await;

        assert!(
            db.get_checkpoint(BRIDGE_ID as u64, CHAIN_ID as u64)
                .await
                .unwrap()
                .is_none(),
            "a NULL previous value must not be treated as a raise, and no checkpoint row exists to write to"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_stale_row_for_removed_address_does_not_influence_decision() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_stale_row_ignored").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        // A stale row for an address no longer in config, with a lowered
        // value that must NOT influence the decision.
        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xFF; 20], 1, None, Some(10)).await;
        // The actually-configured contract has no stored row yet (identity
        // mismatch: different address), so `prev` must read as unknown.
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 500, 999, 2000)
            .await
            .unwrap();

        let config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];

        reconcile_catchup_floors(&db, &config).await;

        assert_eq!(
            checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await,
            500,
            "a stale row for a different address must not drive the floor down to 10"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_amb_pair_only_resets_on_amb_proxy_lowered() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_amb_proxy_vs_mediator").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        store_bridge_contract(
            &db,
            BRIDGE_ID,
            CHAIN_ID,
            &[0xAA; 20],
            1,
            Some("amb_proxy"),
            Some(1000),
        )
        .await;
        store_bridge_contract(
            &db,
            BRIDGE_ID,
            CHAIN_ID,
            &[0xBB; 20],
            1,
            Some("omnibridge_mediator"),
            Some(2000),
        )
        .await;
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 1000, 999, 5000)
            .await
            .unwrap();

        // Lowering only the mediator's value must trigger no reset: the AMB
        // floor rule only ever looks at `amb_proxy`.
        let mediator_lowered = vec![bridge(
            BRIDGE_ID,
            BridgeType::Amb,
            true,
            vec![
                contract_with_address(CHAIN_ID, Some("amb_proxy"), 1000, 1, 0xAA),
                contract_with_address(CHAIN_ID, Some("omnibridge_mediator"), 100, 1, 0xBB),
            ],
        )];
        reconcile_catchup_floors(&db, &mediator_lowered).await;
        assert_eq!(
            checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await,
            1000,
            "lowering only the mediator's started_at_block must not reset the floor"
        );

        // Lowering the amb_proxy value does trigger a reset.
        let amb_proxy_lowered = vec![bridge(
            BRIDGE_ID,
            BridgeType::Amb,
            true,
            vec![
                contract_with_address(CHAIN_ID, Some("amb_proxy"), 400, 1, 0xAA),
                contract_with_address(CHAIN_ID, Some("omnibridge_mediator"), 2000, 1, 0xBB),
            ],
        )];
        reconcile_catchup_floors(&db, &amb_proxy_lowered).await;
        assert_eq!(
            checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await,
            400,
            "lowering amb_proxy's started_at_block must reset the floor"
        );
    }

    /// Acceptance criterion 7: the reconciliation must read `bridge_contracts`
    /// *before* `upsert_bridge_contracts` overwrites it. This test invokes both
    /// in the exact order `server.rs::run` uses and asserts both outcomes: the
    /// lowered value is detected *and* `bridge_contracts` ends up holding the
    /// new value afterward.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_runs_before_upsert_bridge_contracts_detects_lowered_value() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_ordering_before_upsert_bridge_contracts").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xAA; 20], 1, None, Some(1000)).await;
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 1000, 999, 2000)
            .await
            .unwrap();

        let lowered_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];

        // Exact order server.rs::run uses: reconcile, THEN upsert_bridge_contracts.
        reconcile_catchup_floors(&db, &lowered_config).await;
        let bridge_contracts: Vec<bridge_contracts::ActiveModel> = lowered_config
            .iter()
            .flat_map(|bridge| {
                bridge
                    .contracts
                    .iter()
                    .map(move |contract| contract.to_active_model(bridge.bridge_id))
            })
            .collect();
        db.upsert_bridge_contracts(bridge_contracts).await.unwrap();

        // Detection worked (the reset fired before the overwrite).
        assert_eq!(checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await, 500);
        // And bridge_contracts now holds the new value.
        let stored = db.get_bridge_contracts(BRIDGE_ID).await.unwrap();
        let stored_contract = stored
            .iter()
            .find(|row| row.chain_id == CHAIN_ID && row.address == vec![0xAA; 20])
            .expect("stored contract row must exist");
        assert_eq!(stored_contract.started_at_block, Some(500));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_failed_write_does_not_panic_or_propagate() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_failed_write_is_warn_only").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xAA; 20], 1, None, Some(1000)).await;
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 1000, 999, 2000)
            .await
            .unwrap();

        // Sever the connection so every subsequent accessor call fails.
        db.db.close_by_ref().await.unwrap();

        let lowered_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];

        // `reconcile_catchup_floors` never panics or propagates a `Result` --
        // a failure here is reported only via the returned bridge-id set, so
        // this must simply not panic, and the bridge must come back as
        // unreconciled (the connection is closed, so `get_bridge_contracts`
        // itself fails).
        let unreconciled = reconcile_catchup_floors(&db, &lowered_config).await;
        assert!(unreconciled.contains(&BRIDGE_ID));
    }

    /// Regression test for the permanence gap fixed by
    /// `bridges_pending_contracts_upsert`: a bridge whose reconciliation could
    /// not be completed this startup must not have its `bridge_contracts` row
    /// refreshed, or the previous `started_at_block` -- the only place that
    /// value survives -- is overwritten on the very startup that failed to
    /// act on it, so `prev == new` forever after and no later restart can
    /// ever detect the lowering. This test fails if the exclusion in
    /// `bridges_pending_contracts_upsert` is removed.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridges_pending_contracts_upsert_excludes_unreconciled_bridge_and_permits_retry() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("bridges_pending_upsert_excludes_unreconciled").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xAA; 20], 1, None, Some(1000)).await;
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 1000, 999, 2000)
            .await
            .unwrap();

        let lowered_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];

        // Startup #1: an independent connection to the same test database,
        // closed before use, stands in for a transient DB failure during
        // this bridge's reconciliation -- without disturbing `db`'s own
        // connection, so the rest of the test can keep using it.
        let broken_conn = Arc::new(
            sea_orm::Database::connect(test_db.db_url())
                .await
                .expect("connect a second handle to the same test database"),
        );
        broken_conn
            .close_by_ref()
            .await
            .expect("close the second handle before use");
        let failing_db = InterchainDatabase::new(broken_conn);

        let unreconciled = reconcile_catchup_floors(&failing_db, &lowered_config).await;
        assert!(
            unreconciled.contains(&BRIDGE_ID),
            "a failed read or write during reconciliation must mark the bridge unreconciled"
        );

        let pending = bridges_pending_contracts_upsert(&lowered_config, &unreconciled);
        assert!(
            pending.is_empty(),
            "an unreconciled bridge must be excluded from this startup's bridge_contracts upsert"
        );
        let bridge_contracts_payload: Vec<bridge_contracts::ActiveModel> = pending
            .into_iter()
            .flat_map(|bridge| {
                bridge
                    .contracts
                    .iter()
                    .map(move |contract| contract.to_active_model(bridge.bridge_id))
            })
            .collect();
        if !bridge_contracts_payload.is_empty() {
            db.upsert_bridge_contracts(bridge_contracts_payload)
                .await
                .unwrap();
        }

        // bridge_contracts must still hold the OLD value: nothing refreshed it.
        let stored = db.get_bridge_contracts(BRIDGE_ID).await.unwrap();
        let stored_contract = stored
            .iter()
            .find(|row| row.chain_id == CHAIN_ID && row.address == vec![0xAA; 20])
            .expect("stored contract row must exist");
        assert_eq!(
            stored_contract.started_at_block,
            Some(1000),
            "bridge_contracts must still hold the previous run's value; a failed \
             reconciliation must not let it be overwritten"
        );
        // And the floor was never lowered on this failed startup.
        assert_eq!(checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await, 1000);

        // Startup #2, same config, on a healthy connection: because
        // bridge_contracts still holds the pre-lowering value, reconciliation
        // now detects the lowering and fixes the floor.
        let unreconciled_second = reconcile_catchup_floors(&db, &lowered_config).await;
        assert!(unreconciled_second.is_empty());
        assert_eq!(
            checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await,
            500,
            "the second startup must retry and detect the lowering the first startup could \
             not confirm"
        );
    }

    /// Regression test for the disabled-bridge deviation documented on
    /// `reconcile_catchup_floors`: `server.rs` upserts `bridge_contracts` for
    /// every configured bridge regardless of `enabled`, so reconciliation
    /// must cover disabled bridges too, or a disable → lower
    /// `started_at_block` → restart → re-enable → restart sequence consumes
    /// the previous value with nothing having looked at it, and the stored
    /// floor never lowers.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_disabled_bridge_lowered_then_reenabled_ends_up_lowered() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_disabled_bridge_lowered_reenabled").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        // Prior state: the bridge ran enabled with started_at_block = 1000
        // and completed catch-up.
        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xAA; 20], 1, None, Some(1000)).await;
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 1000, 999, 2000)
            .await
            .unwrap();

        // Restart #1: the operator disables the bridge AND lowers
        // started_at_block to 500 in the same config change.
        let disabled_lowered_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            false,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];

        let unreconciled = reconcile_catchup_floors(&db, &disabled_lowered_config).await;
        assert!(unreconciled.is_empty());

        // Mirrors server.rs: `bridges_pending_contracts_upsert` excludes only
        // unreconciled bridges, never disabled ones.
        let bridge_contracts_payload: Vec<bridge_contracts::ActiveModel> =
            bridges_pending_contracts_upsert(&disabled_lowered_config, &unreconciled)
                .into_iter()
                .flat_map(|bridge| {
                    bridge
                        .contracts
                        .iter()
                        .map(move |contract| contract.to_active_model(bridge.bridge_id))
                })
                .collect();
        db.upsert_bridge_contracts(bridge_contracts_payload)
            .await
            .unwrap();

        assert_eq!(
            checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await,
            500,
            "a disabled bridge must still be reconciled before its bridge_contracts row is \
             refreshed, or the lowering is lost permanently"
        );

        // Restart #2: the operator re-enables the bridge; started_at_block is
        // unchanged from restart #1.
        let reenabled_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];
        let unreconciled_second = reconcile_catchup_floors(&db, &reenabled_config).await;
        assert!(unreconciled_second.is_empty());

        assert_eq!(
            checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await,
            500,
            "the floor must end up lowered and stay lowered after re-enabling"
        );
    }
}
