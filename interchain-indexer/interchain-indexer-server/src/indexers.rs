// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{BridgeConfig, BridgeContractConfig, ChainConfig, Settings, config::IndexerType};
use alloy::{network::Ethereum, primitives::Address, providers::DynProvider};
use anyhow::{Context, Result};
use interchain_indexer_entity::sea_orm_active_enums::BridgeType;
use interchain_indexer_logic::{
    CrosschainIndexer, InterchainDatabase, StatsService,
    indexer::{
        amb::{AmbChainConfig, AmbContractConfig, AmbIndexer},
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
                log_amb_floor_divergence(bridge);
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
    let mut chain_configs = Vec::new();
    for plan in plan_bridge(bridge) {
        let chain_id = plan.chain_id;
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

        // Every configured version of each kind, not the first of each: an
        // upgrade behind the same address is expressed as another entry with a
        // higher version and the block it takes effect from. Taking one per
        // kind silently dropped the rest.
        let amb_proxies =
            amb_contract_configs(bridge.bridge_id, chain_id, AMB_PROXY_KIND, &plan.contracts);
        let mediators = amb_contract_configs(
            bridge.bridge_id,
            chain_id,
            OMNIBRIDGE_MEDIATOR_KIND,
            &plan.contracts,
        );

        if amb_proxies.is_empty() {
            tracing::error!(
                bridge_id = bridge.bridge_id,
                chain_id,
                "AMB bridge requires at least one usable amb_proxy contract per chain"
            );
            continue;
        }

        // Mediators are optional by design: with none configured this chain's
        // messages are indexed without token transfers.
        if mediators.is_empty() {
            tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id,
                "no usable omnibridge_mediator contract; indexing messages without transfers"
            );
        }

        // From the plan, not from a contract read here: the progress API and
        // the startup floor reconciliation read the same `ChainPlan`, so the
        // denominator cannot drift from where the indexer actually starts.
        let Some(start_block) = plan.start_block() else {
            continue;
        };

        chain_configs.push(AmbChainConfig {
            chain_id,
            provider: provider.clone(),
            start_block,
            amb_proxies,
            mediators,
        });
    }

    chain_configs
}

/// Every configured contract of one `kind` on a chain, as the indexer's own
/// config type. Entries whose address does not parse are dropped with an error
/// — the same treatment they got before, now per entry rather than per kind.
fn amb_contract_configs(
    bridge_id: i32,
    chain_id: i64,
    kind: &str,
    contracts: &[&BridgeContractConfig],
) -> Vec<AmbContractConfig> {
    contracts
        .iter()
        .filter(|contract| contract.kind.as_deref() == Some(kind))
        .filter_map(|contract| {
            let address = parse_contract_address(bridge_id, chain_id, kind, &contract.address)?;
            Some(AmbContractConfig {
                address,
                version: contract.version,
                started_at_block: contract.started_at_block,
                abi: parse_contract_abi(bridge_id, chain_id, kind, contract.abi.as_ref()),
            })
        })
        .collect()
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

    for plan in plan_bridge(bridge) {
        let chain_id = plan.chain_id;
        let Some(_chain_config) = chain_lookup.get(&chain_id) else {
            tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id,
                "Chain configuration missing for Avalanche indexer"
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

        let mut contract_addresses = Vec::new();
        for contract in &plan.contracts {
            let Ok(address_bytes): Result<[u8; 20], _> = contract.address.clone().try_into() else {
                tracing::error!(
                    bridge_id = bridge.bridge_id,
                    chain_id,
                    version = contract.version,
                    "Bridge contract address must be 20 bytes"
                );
                continue;
            };
            contract_addresses.push(Address::from(address_bytes));
        }

        // Every configured address on this chain is scanned by one stream,
        // from one floor. See `AvalancheChainConfig` for why per-contract
        // streams over a shared `(bridge_id, chain_id)` checkpoint lose data.
        let Some(start_block) = plan.start_block() else {
            tracing::error!(
                bridge_id = bridge.bridge_id,
                chain_id,
                "no scan floor could be derived for this pair; skipping chain"
            );
            continue;
        };

        if contract_addresses.is_empty() {
            tracing::error!(
                bridge_id = bridge.bridge_id,
                chain_id,
                "no usable contract address for this pair; skipping chain"
            );
            continue;
        }

        chain_configs.push(AvalancheChainConfig {
            chain_id,
            start_block,
            provider: provider.clone(),
            contract_addresses,
        });
    }

    chain_configs
}

// --- Indexing-progress API support ---
//
// `plan_bridge`, `enumerate_indexing_targets` and
// `reconcile_catchup_floors` are the config-driven surface behind
// `GetIndexingProgress`. See `.memory-bank/research/message-lifecycle.md` §2
// for the cursor semantics these functions rely on.

/// `kind` values AMB assigns to its contracts. Parsing and interpreting `kind`
/// is the AMB indexer's business; these constants exist because the *plan*
/// must know which contracts drive a chain's scan.
pub(crate) const AMB_PROXY_KIND: &str = "amb_proxy";
pub(crate) const OMNIBRIDGE_MEDIATOR_KIND: &str = "omnibridge_mediator";

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

/// One `(bridge, chain)` pair as its protocol sees it: every contract configured
/// on that chain, plus the subset whose `started_at_block` determines where the
/// scan begins.
///
/// This exists so "where does this pair start" is decided **once**. The rule
/// previously lived in three places — `build_amb_chain_configs`,
/// `scan_floor_for_pair` and `derive_prev_floor` — each re-deriving it from
/// `kind`, with a comment asking future readers to keep them in lock-step and a
/// test whose only job was to catch the drift. Drift there does not fail: it
/// silently mis-reports a denominator, or reconciles a floor against a
/// different contract than the one the indexer actually starts from.
pub(crate) struct ChainPlan<'a> {
    pub(crate) chain_id: i64,
    /// Every configured contract on this chain. The set of addresses the
    /// indexer collects logs from is derived from this by the indexer itself.
    pub(crate) contracts: Vec<&'a BridgeContractConfig>,
    /// The contracts whose `started_at_block` sets the floor. Empty when the
    /// protocol's required contracts are absent from the config.
    pub(crate) floor_contracts: Vec<&'a BridgeContractConfig>,
}

impl ChainPlan<'_> {
    /// The block handed to `LogStream.genesis_block`, or `None` when the pair
    /// has no contract that could define one.
    ///
    /// `None` is not the same as "not configured": the pair is still enumerated
    /// for the progress API, because a pair whose indexer cannot start is
    /// exactly what that endpoint exists to surface. It just gets no indexer.
    pub(crate) fn start_block(&self) -> Option<u64> {
        self.floor_contracts
            .iter()
            .map(|contract| contract.started_at_block)
            .min()
    }
}

/// Groups `bridge`'s contracts into one plan per chain, applying that bridge
/// type's own rule for which contracts define the floor.
///
/// **AMB** scans from the earliest `amb_proxy`, never from the mediator. In
/// `config/omnibridge/bridges.json` the mediator sits ~7.4M blocks below the
/// proxy on chain 1, so `min` across all contracts would badly understate the
/// denominator — and the indexer would not scan there anyway. A chain with no
/// `amb_proxy` yields an empty `floor_contracts`: AMB cannot index it at all,
/// and inferring a floor from the mediator would produce a plausible number for
/// a scan that never happens. The mediator is *not* required — a bridge
/// configured with AMB contracts only indexes messages without transfers.
///
/// **Everything else** takes `min` across the chain's contracts. Avalanche
/// declares one contract per chain today, so `min` is that contract's value;
/// with several deployments on one chain it is the earliest, which is the only
/// floor under which every configured address is covered.
///
/// Pure, and deliberately so: all three callers run at startup, and a condition
/// logged here would be reported once per caller rather than once.
pub(crate) fn plan_bridge(bridge: &BridgeConfig) -> Vec<ChainPlan<'_>> {
    let mut plans: Vec<ChainPlan<'_>> = group_contracts_by_chain(&bridge.contracts)
        .into_iter()
        .map(|(chain_id, contracts)| {
            let floor_contracts = match bridge.bridge_type {
                BridgeType::Amb => contracts
                    .iter()
                    .copied()
                    .filter(|contract| contract.kind.as_deref() == Some(AMB_PROXY_KIND))
                    .collect(),
                _ => contracts.clone(),
            };

            ChainPlan {
                chain_id,
                contracts,
                floor_contracts,
            }
        })
        .collect();

    plans.sort_by_key(|plan| plan.chain_id);
    plans
}

/// Warns when a chain's contract kinds start at materially different blocks, so
/// the range that is consequently not covered end-to-end is visible in the log
/// rather than only derivable from the config.
///
/// Both directions matter and they are different problems. A mediator starting
/// *below* the proxy means its logs under the proxy's floor are never fetched.
/// A mediator starting *above* it means the range in between indexes messages
/// with no transfer contract configured — accepted by design (AMB-only
/// operation is supported), but the operator should know the range.
///
/// Startup-only: called once per running AMB bridge, not from `plan_bridge`.
fn log_amb_floor_divergence(bridge: &BridgeConfig) {
    for plan in plan_bridge(bridge) {
        let Some(proxy_floor) = plan.start_block() else {
            continue;
        };
        let mediator_floor = plan
            .contracts
            .iter()
            .filter(|contract| contract.kind.as_deref() == Some(OMNIBRIDGE_MEDIATOR_KIND))
            .map(|contract| contract.started_at_block)
            .min();

        match mediator_floor {
            None => tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id = plan.chain_id,
                proxy_floor,
                "AMB chain has no omnibridge_mediator contract: messages will be indexed \
                 without token transfers"
            ),
            Some(mediator_floor) if mediator_floor < proxy_floor => tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id = plan.chain_id,
                proxy_floor,
                mediator_floor,
                excluded_from = mediator_floor,
                excluded_to = proxy_floor - 1,
                "omnibridge_mediator starts below the amb_proxy floor: its logs in that \
                 range are never scanned"
            ),
            Some(mediator_floor) if mediator_floor > proxy_floor => tracing::warn!(
                bridge_id = bridge.bridge_id,
                chain_id = plan.chain_id,
                proxy_floor,
                mediator_floor,
                messages_without_transfers_from = proxy_floor,
                messages_without_transfers_to = mediator_floor - 1,
                "omnibridge_mediator starts above the amb_proxy floor: messages in that \
                 range are indexed without token transfers"
            ),
            Some(_) => {}
        }
    }
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
/// AMB chain with no `amb_proxy` contract (`ChainPlan::start_block` is then `None` and the
/// pair is reported from block 0) — because a pair whose indexer failed to start is exactly
/// the case this enumeration exists to surface at 0% instead of silently omitting it.
///
/// Sorted by `(bridge_id, chain_id)`.
pub fn enumerate_indexing_targets(bridges: &[BridgeConfig]) -> Vec<IndexingTarget> {
    let mut targets = Vec::new();

    for bridge in bridges {
        if !bridge.enabled {
            continue;
        }

        for plan in plan_bridge(bridge) {
            let start_block = match plan.start_block() {
                Some(start_block) => start_block,
                None => {
                    tracing::warn!(
                        bridge_id = bridge.bridge_id,
                        chain_id = plan.chain_id,
                        "no scan floor could be derived for this configured pair; reporting start_block = 0"
                    );
                    0
                }
            };

            targets.push(IndexingTarget {
                bridge_id: bridge.bridge_id,
                chain_id: plan.chain_id,
                start_block,
            });
        }
    }

    targets.sort_by_key(|target| (target.bridge_id, target.chain_id));
    targets
}

/// Enforces the invariant `catchup_min_cursor == the pair's configured scan floor`
/// by lowering the stored value whenever configuration sits below it.
///
/// # Why this is allowed to be this simple
///
/// `catchup_min_cursor` is **not** a scan frontier today. It never advances with
/// progress: the cursor-maintenance writer always supplies `0` and relies on
/// `GREATEST(existing, 0)` purely to preserve whatever is there
/// (`message_buffer/persistence.rs`). Exactly two writers move it —
/// `seed_catchup_floor` (raise-only) and `lower_catchup_floor` (lower-only) — so the
/// stored value *is* the previous run's configured floor, and comparing configuration
/// against it directly is exact rather than a proxy.
///
/// That is the whole licence for this function. An earlier design reconstructed the
/// previous floor from `bridge_contracts.started_at_block` instead, in order to act
/// only on a detected config *transition*. It could not survive a change to the
/// identity set: adding a contract whose `started_at_block` is below the current floor
/// leaves the new `(address, version)` without a stored row, the derived previous floor
/// reads as unknown, nothing is lowered, and the same startup's
/// `upsert_bridge_contracts` then writes the new value — after which the derived
/// previous floor equals the configured one forever and the pair is pinned at the old
/// floor permanently. Comparing against the checkpoint removes the proxy, and with it
/// that failure, the ordering constraint against `upsert_bridge_contracts`, and the
/// need to withhold contract rows to preserve evidence.
///
/// # This is a workaround, and here is its expiry condition
///
/// Enforcing `catchup_min_cursor == configured floor` is only correct while catch-up is
/// **one-directional**. Under a bidirectional catch-up, `catchup_min_cursor` becomes the
/// ascending frontier, so `configured_floor < stored` is true whenever the ascending walk
/// has made any progress — this function would then fire on *every* startup and reset that
/// walk each time. Not a one-off rescan: a loop.
///
/// The correct shape there is to persist the floor in its own column, separate from the
/// frontier, and reconcile against that. Then a lowered floor is applied by lowering the
/// stored floor and setting `catchup_max_cursor` to `old_floor - 1`, which confines the
/// rescan to exactly the newly opened range. That needs a migration, which is why it is
/// not done here.
///
/// **Whoever makes `catchup_min_cursor` a real scan boundary must replace this function,
/// not extend it** — and must also make its write failure fatal for the pair. Today a
/// failed write is a `warn` because the downward scan takes its floor from the config value
/// handed to `LogStream.genesis_block` and ignores `catchup_min_cursor` entirely, so the
/// cost is a wrong reading on the progress endpoint and no lost data. Under a bidirectional
/// indexer the identical failure would silently drop the newly opened range instead.
///
/// # Notes
///
/// No rescan is caused by lowering the floor in the current design. A completed catch-up
/// left `catchup_max_cursor` at `old_floor - 1` (`mark_catchup_complete`), so the descending
/// scan resumes exactly at the boundary; a catch-up still in progress has that cursor above
/// the old floor and simply keeps walking past it.
///
/// The decision itself lives in SQL: `lower_catchup_floor` is
/// `SET catchup_min_cursor = new WHERE catchup_min_cursor > new`. Calling it
/// unconditionally is therefore both the raise-guard and the no-op case, and re-running it
/// on an already-correct pair writes nothing.
///
/// Runs for **every** bridge, including disabled ones: a disabled bridge can be lowered and
/// re-enabled later, nothing indexes it in the meantime, and `enumerate_indexing_targets`
/// still excludes it from the progress endpoint. Never panics or propagates: each pair's
/// failure is logged and skipped, and the next startup retries it — the invariant is
/// re-enforced on every startup rather than detected once.
pub async fn reconcile_catchup_floors(db: &InterchainDatabase, bridges: &[BridgeConfig]) {
    for bridge in bridges {
        for plan in plan_bridge(bridge) {
            let chain_id = plan.chain_id;
            let Some(floor) = plan.start_block() else {
                tracing::warn!(
                    bridge_id = bridge.bridge_id,
                    chain_id,
                    "no scan floor could be derived for this configured pair; skipping catchup floor reconciliation"
                );
                continue;
            };

            match db
                .lower_catchup_floor(bridge.bridge_id, chain_id, floor)
                .await
            {
                // `rows_affected == 0` is the common case: the stored floor already
                // matches configuration, or configuration was raised and the seed's
                // `GREATEST` owns that direction.
                Ok(0) => {}
                Ok(rows_affected) => tracing::info!(
                    bridge_id = bridge.bridge_id,
                    chain_id,
                    floor,
                    rows_affected,
                    "lowered stored catchup floor to the configured scan floor"
                ),
                Err(err) => tracing::warn!(
                    err = ?err,
                    bridge_id = bridge.bridge_id,
                    chain_id,
                    floor,
                    "failed to lower catchup floor; startup continues and the next one retries, \
                     progress reporting may under-state this pair until then"
                ),
            }
        }
    }
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

    fn chain_config_fixture(chain_id: i64) -> ChainConfig {
        ChainConfig {
            chain_id,
            name: format!("chain-{chain_id}"),
            icon: String::new(),
            explorer: Default::default(),
            pool_config: Default::default(),
            rpcs: Vec::new(),
        }
    }

    /// Never dialed: the config builders only clone the handle.
    fn dummy_provider() -> DynProvider<Ethereum> {
        use alloy::providers::{Provider, ProviderBuilder};
        ProviderBuilder::new()
            .connect_http("http://127.0.0.1:1".parse().unwrap())
            .erased()
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

    // --- plan_bridge / enumerate_indexing_targets ---

    #[test]
    fn test_enumerate_indexing_targets_amb_config_pins_amb_proxy_floors_not_mediator() {
        // Drift between the denominator and where the indexer actually starts
        // is now structurally impossible — both read `ChainPlan` — so this
        // pins the other half: that the shared rule produces the right floors
        // for the *real* config, and that a future edit to `plan_bridge`
        // cannot quietly start preferring the mediator.
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

    /// A chain declaring several `amb_proxy` versions starts from the earliest,
    /// not from whichever the config happens to list first. This is the rule
    /// versioned deployments rely on: adding a v2 entry must not move the floor
    /// up and orphan everything v1 covered.
    #[test]
    fn test_plan_bridge_amb_floor_is_the_earliest_amb_proxy_version() {
        let bridge = bridge(
            1,
            BridgeType::Amb,
            true,
            vec![
                contract(1, Some("amb_proxy"), 900, 2),
                contract(1, Some("amb_proxy"), 100, 1),
                contract(1, Some("omnibridge_mediator"), 50, 1),
            ],
        );

        let plans = plan_bridge(&bridge);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].start_block(), Some(100));
        assert_eq!(
            plans[0].contracts.len(),
            3,
            "every configured contract stays in the plan; only the floor is selective"
        );
    }

    /// An AMB chain without an `amb_proxy` cannot be indexed at all, so it has
    /// no floor — rather than silently inheriting the mediator's, which would
    /// report a plausible denominator for a scan that never runs.
    #[test]
    fn test_plan_bridge_amb_without_proxy_has_no_floor() {
        let bridges = vec![bridge(
            1,
            BridgeType::Amb,
            true,
            vec![contract(1, Some("omnibridge_mediator"), 50, 1)],
        )];

        assert_eq!(plan_bridge(&bridges[0])[0].start_block(), None);

        // Still enumerated: a misconfigured pair must surface at 0%, not vanish.
        assert_eq!(
            enumerate_indexing_targets(&bridges),
            vec![IndexingTarget {
                bridge_id: 1,
                chain_id: 1,
                start_block: 0
            }]
        );
    }

    /// The mediator is optional by design: AMB contracts alone index messages
    /// without token transfers, so their absence must not remove the floor.
    #[test]
    fn test_plan_bridge_amb_without_mediator_keeps_its_floor() {
        let bridge = bridge(
            1,
            BridgeType::Amb,
            true,
            vec![contract(1, Some("amb_proxy"), 100, 1)],
        );

        assert_eq!(plan_bridge(&bridge)[0].start_block(), Some(100));
    }

    /// Every configured version of each kind must reach the indexer. Taking
    /// one per kind — which is what `.find()` did — silently indexed whichever
    /// the config listed first and dropped the upgrade.
    #[test]
    fn test_build_amb_chain_configs_keeps_every_configured_version() {
        let bridge = bridge(
            1,
            BridgeType::Amb,
            true,
            vec![
                BridgeContractConfig {
                    address: vec![0xAA; 20],
                    ..contract(1, Some(AMB_PROXY_KIND), 100, 1)
                },
                // Same address, later block: an implementation upgrade behind
                // the proxy, which is the normal AMB deployment shape.
                BridgeContractConfig {
                    address: vec![0xAA; 20],
                    ..contract(1, Some(AMB_PROXY_KIND), 900, 2)
                },
                BridgeContractConfig {
                    address: vec![0xBB; 20],
                    ..contract(1, Some(OMNIBRIDGE_MEDIATOR_KIND), 50, 1)
                },
            ],
        );

        let chain_lookup = HashMap::from([(1i64, chain_config_fixture(1))]);
        let chain_providers = HashMap::from([(1i64, dummy_provider())]);

        let configs = build_amb_chain_configs(&bridge, &chain_lookup, &chain_providers);

        assert_eq!(configs.len(), 1);
        assert_eq!(
            configs[0]
                .amb_proxies
                .iter()
                .map(|proxy| (proxy.version, proxy.started_at_block))
                .collect::<Vec<_>>(),
            vec![(1, 100), (2, 900)]
        );
        assert_eq!(configs[0].mediators.len(), 1);
        assert_eq!(
            configs[0].start_block, 100,
            "the scan starts at the earliest proxy version, not the latest"
        );
    }

    /// A chain configured with AMB contracts only is indexable: messages
    /// without token transfers. Before, the missing mediator disqualified the
    /// whole pair and nothing on that chain was indexed at all.
    #[test]
    fn test_build_amb_chain_configs_without_mediator_still_indexes_the_chain() {
        let bridge = bridge(
            1,
            BridgeType::Amb,
            true,
            vec![contract(1, Some(AMB_PROXY_KIND), 100, 1)],
        );

        let chain_lookup = HashMap::from([(1i64, chain_config_fixture(1))]);
        let chain_providers = HashMap::from([(1i64, dummy_provider())]);

        let configs = build_amb_chain_configs(&bridge, &chain_lookup, &chain_providers);

        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].amb_proxies.len(), 1);
        assert!(configs[0].mediators.is_empty());
    }

    /// Without a proxy there is nothing to scan from, so no config is produced
    /// — the mediator alone cannot drive an AMB chain.
    #[test]
    fn test_build_amb_chain_configs_without_proxy_produces_nothing() {
        let bridge = bridge(
            1,
            BridgeType::Amb,
            true,
            vec![contract(1, Some(OMNIBRIDGE_MEDIATOR_KIND), 50, 1)],
        );

        let chain_lookup = HashMap::from([(1i64, chain_config_fixture(1))]);
        let chain_providers = HashMap::from([(1i64, dummy_provider())]);

        assert!(build_amb_chain_configs(&bridge, &chain_lookup, &chain_providers).is_empty());
    }

    /// A chain with several Teleporter deployments must produce **one**
    /// `AvalancheChainConfig` carrying every address, not one per contract.
    /// Per-contract configs become per-contract log streams sharing a single
    /// `(bridge_id, chain_id)` checkpoint: whichever finishes catch-up first
    /// lowers the pair's `catchup_max_cursor`, and after a restart the others
    /// resume from it and never scan what they had left — with no RPC failure
    /// and no ledger row, because every shared record says those blocks were
    /// scanned.
    #[test]
    fn test_build_avalanche_chain_configs_groups_a_chain_s_contracts_into_one_stream() {
        let bridge = bridge(
            1,
            BridgeType::AvalancheNative,
            true,
            vec![
                BridgeContractConfig {
                    address: vec![0xAA; 20],
                    started_at_block: 900,
                    ..contract(1, None, 900, 2)
                },
                BridgeContractConfig {
                    address: vec![0xBB; 20],
                    started_at_block: 100,
                    ..contract(1, None, 100, 1)
                },
            ],
        );

        let chain_lookup = HashMap::from([(1i64, chain_config_fixture(1))]);
        let chain_providers = HashMap::from([(1i64, dummy_provider())]);

        let configs = build_avalanche_chain_configs(&bridge, &chain_lookup, &chain_providers);

        assert_eq!(configs.len(), 1, "one stream per chain, not per contract");
        assert_eq!(configs[0].chain_id, 1);
        assert_eq!(
            configs[0].contract_addresses,
            vec![Address::from([0xAA; 20]), Address::from([0xBB; 20])]
        );
        assert_eq!(
            configs[0].start_block, 100,
            "the shared stream must start low enough to cover every address"
        );
    }

    /// Non-AMB bridges take `min` across every contract on the chain — the only
    /// floor under which each configured address is covered.
    #[test]
    fn test_plan_bridge_non_amb_floor_is_min_across_contracts() {
        let bridge = bridge(
            1,
            BridgeType::AvalancheNative,
            true,
            vec![contract(1, None, 900, 2), contract(1, None, 100, 1)],
        );

        let plans = plan_bridge(&bridge);
        assert_eq!(plans[0].start_block(), Some(100));
        assert_eq!(plans[0].floor_contracts.len(), 2);
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

    // --- reconcile_catchup_floors (DB-backed) ---

    use blockscout_service_launcher::test_database::TestDbGuard;
    use interchain_indexer_entity::{bridge_contracts, bridges, chains};
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

    /// Stores a `bridge_contracts` row directly. Nothing in the reconciliation
    /// reads this table any more; the fixtures that still use it exist
    /// precisely to prove that its contents cannot influence the outcome.
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

    /// Mirrors what `server.rs` does after the reconciliation.
    async fn upsert_configured_contracts(db: &InterchainDatabase, bridges: &[BridgeConfig]) {
        let payload: Vec<bridge_contracts::ActiveModel> = bridges
            .iter()
            .flat_map(|bridge| {
                bridge
                    .contracts
                    .iter()
                    .map(move |contract| contract.to_active_model(bridge.bridge_id))
            })
            .collect();
        db.upsert_bridge_contracts(payload).await.unwrap();
    }

    async fn progress_for(
        db: &InterchainDatabase,
        bridge_id: i32,
        chain_id: i64,
        start_block: u64,
    ) -> CatchupProgress {
        let checkpoint = db
            .get_checkpoint(bridge_id as u64, chain_id as u64)
            .await
            .unwrap()
            .expect("checkpoint row must exist");
        CatchupProgress::compute(
            start_block,
            Some(CheckpointCursors {
                catchup_min_cursor: checkpoint.validated_catchup_min_cursor(),
                catchup_max_cursor: checkpoint.validated_catchup_cursor(),
                realtime_cursor: checkpoint.validated_realtime_cursor(),
            }),
        )
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

        // Previous run: floor 1000, catch-up already completed (X = 999).
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 1000, 999, 2000)
            .await
            .unwrap();

        // Without reconciliation, the stale floor (1000) against the new, lower
        // start_block (500) reports the widened range as complete -- exactly
        // the bug this feature exists to fix.
        let stale_progress = progress_for(&db, BRIDGE_ID, CHAIN_ID, 500).await;
        assert!(stale_progress.scan_complete);
        assert_eq!(stale_progress.blocks_remaining, 0);

        let new_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];
        reconcile_catchup_floors(&db, &new_config).await;

        let checkpoint = db
            .get_checkpoint(BRIDGE_ID as u64, CHAIN_ID as u64)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.catchup_min_cursor, 500);
        assert_eq!(
            checkpoint.catchup_max_cursor, 999,
            "reconciliation must not touch catchup_max_cursor: a completed catch-up already \
             left it at old_floor - 1, which is exactly where the reopened scan resumes"
        );

        let fixed_progress = progress_for(&db, BRIDGE_ID, CHAIN_ID, 500).await;
        assert!(!fixed_progress.scan_complete);
        assert_eq!(fixed_progress.blocks_remaining, 500);
    }

    /// The review finding this rewrite exists for: adding a contract identity
    /// whose `started_at_block` sits below the current floor. The previous
    /// design derived the previous floor from `bridge_contracts` and read
    /// "unknown" here, because the new `(address, version)` has no stored row
    /// yet -- so it did nothing, and the same startup's
    /// `upsert_bridge_contracts` then destroyed the only evidence that the pair
    /// used to start higher, pinning it at the old floor permanently.
    ///
    /// Comparing configuration against `catchup_min_cursor` instead fixes it on
    /// the *first* startup, and the second startup is asserted too because that
    /// is where the old design was unrecoverable.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_new_lower_floor_identity_is_applied_on_the_first_startup() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_new_lower_identity").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        // Prior state: one identity at 1000, its row stored, catch-up complete.
        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xAA; 20], 1, None, Some(1000)).await;
        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 1000, 999, 2000)
            .await
            .unwrap();

        // Config gains a second identity starting at 500. The old entry stays.
        let widened_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![
                contract_with_address(CHAIN_ID, None, 1000, 1, 0xAA),
                contract_with_address(CHAIN_ID, None, 500, 1, 0xBB),
            ],
        )];

        // Startup #1, in server.rs order: reconcile, then refresh contracts.
        reconcile_catchup_floors(&db, &widened_config).await;
        upsert_configured_contracts(&db, &widened_config).await;

        assert_eq!(
            checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await,
            500,
            "a newly added identity below the current floor must lower it immediately"
        );
        let progress = progress_for(&db, BRIDGE_ID, CHAIN_ID, 500).await;
        assert!(
            !progress.scan_complete,
            "the reopened range must not read as complete while it is being rescanned"
        );
        assert_eq!(progress.blocks_remaining, 500);

        // Startup #2: both identities now have stored rows -- the state in
        // which the old design was permanently stuck. Nothing more to do, and
        // nothing undone.
        reconcile_catchup_floors(&db, &widened_config).await;
        assert_eq!(checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await, 500);
        assert_eq!(
            progress_for(&db, BRIDGE_ID, CHAIN_ID, 500)
                .await
                .blocks_remaining,
            500
        );
    }

    /// The ordering constraint the old design carried, asserted inverted: the
    /// contracts refresh runs *first* here, and the floor is still lowered.
    /// `bridge_contracts` is not an input any more, so no call order can break
    /// detection.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_is_independent_of_the_contracts_upsert_order() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_independent_of_upsert_order").await;
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

        // Deliberately the order that broke the old design.
        upsert_configured_contracts(&db, &lowered_config).await;
        reconcile_catchup_floors(&db, &lowered_config).await;

        assert_eq!(checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await, 500);
    }

    /// `bridge_contracts` rows cannot influence the outcome, however stale:
    /// this fixture stores a row for an address no longer in config carrying a
    /// far lower value, and the floor must stay at the configured one.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_stale_contract_rows_cannot_influence_the_floor() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_stale_rows_ignored").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xFF; 20], 1, None, Some(10)).await;
        // A second stale row with a NULL value: the fixture that used to read
        // as "previous unknown" and suppress the write entirely.
        store_bridge_contract(&db, BRIDGE_ID, CHAIN_ID, &[0xEE; 20], 1, None, None).await;
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
            "neither a stale lower value nor a NULL one may reach the decision"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_unchanged_config_value_writes_nothing() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_unchanged_leaves_floor").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

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
    async fn reconcile_catchup_floors_amb_pair_only_resets_on_amb_proxy_lowered() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_amb_proxy_vs_mediator").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

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

    /// A failed write is a `warn`, and the next startup retries it. The old
    /// design needed `bridges_pending_contracts_upsert` to withhold contract
    /// rows so the retry could still detect the change; with the checkpoint as
    /// the input there is no evidence to preserve, so the retry is inherent.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_failed_write_is_warn_only_and_retried_next_startup() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_failed_write_is_warn_only").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

        db.seed_catchup_floor(BRIDGE_ID, CHAIN_ID, 1000, 999, 2000)
            .await
            .unwrap();

        let lowered_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];

        // Startup #1: an independent handle to the same test database, closed
        // before use, stands in for a transient DB failure -- without
        // disturbing `db`'s own connection.
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

        // Must not panic and must not propagate.
        reconcile_catchup_floors(&failing_db, &lowered_config).await;
        assert_eq!(
            checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await,
            1000,
            "a failed write must leave the stored floor alone"
        );

        // Startup #2 on a healthy connection: nothing had to be preserved for
        // this to work.
        reconcile_catchup_floors(&db, &lowered_config).await;
        assert_eq!(checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await, 500);
    }

    /// Disabled bridges are reconciled too. Under the old design that was
    /// load-bearing because the contracts refresh covered disabled bridges and
    /// would consume the previous value unseen; it now simply means a bridge
    /// lowered while disabled is already correct when it is re-enabled.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn reconcile_catchup_floors_disabled_bridge_lowered_then_reenabled_ends_up_lowered() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let test_db = init_db("reconcile_disabled_bridge_lowered_reenabled").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_and_chain(&db, BRIDGE_ID, CHAIN_ID).await;

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
        reconcile_catchup_floors(&db, &disabled_lowered_config).await;
        upsert_configured_contracts(&db, &disabled_lowered_config).await;

        assert_eq!(checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await, 500);

        // Restart #2: re-enabled, started_at_block unchanged.
        let reenabled_config = vec![bridge(
            BRIDGE_ID,
            BridgeType::AvalancheNative,
            true,
            vec![contract_with_address(CHAIN_ID, None, 500, 1, 0xAA)],
        )];
        reconcile_catchup_floors(&db, &reenabled_config).await;

        assert_eq!(
            checkpoint_floor(&db, BRIDGE_ID, CHAIN_ID).await,
            500,
            "the floor must end up lowered and stay lowered after re-enabling"
        );
    }
}
