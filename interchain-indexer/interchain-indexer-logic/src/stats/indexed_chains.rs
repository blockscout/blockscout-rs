// SPDX-License-Identifier: LicenseRef-Blockscout

//! [`IndexedChains`]: the single per-bridge "can this evidence still arrive?"
//! predicate for the stats layer, plus the shared SQL condition builders that
//! spell it out for `projection.rs` and `database.rs` backfill queries.
//!
//! See `.memory-bank/adr/004-stats-observability-horizon-and-asset-union-find.md`
//! Decision 5 for the rationale behind the asymmetry between an absent bridge
//! and a bridge present with an empty chain set.

use std::collections::{BTreeSet, HashMap, HashSet};

use interchain_indexer_entity::{crosschain_messages, crosschain_transfers};
use sea_orm::{
    Condition,
    sea_query::{Expr, IntoColumnRef},
};

/// Which chains a bridge actually indexes, i.e. where its events can be
/// observed at all. The single input that lets the stats layer answer
/// "can the missing evidence still arrive?" without knowing any bridge type.
#[derive(Clone, Debug, Default)]
pub enum IndexedChains {
    /// Treat every chain as indexed for every bridge: missing evidence can
    /// always still arrive, so nothing is committed on partial data. This is
    /// the conservative default for tests and for embedders with no bridge
    /// config; it reduces the new rules to plain finality plus deferral.
    #[default]
    AllIndexed,
    /// Authoritative per-bridge sets, derived from the in-memory bridges config.
    PerBridge(HashMap<i32, HashSet<i64>>),
}

impl IndexedChains {
    /// Builds `PerBridge` from **per-bridge groups**, so a bridge declared with
    /// zero contracts is inserted with an **empty set** rather than being
    /// omitted. This is the constructor `server.rs` must use: a flat
    /// `(bridge_id, chain_id)` pair stream cannot represent a contract-less
    /// bridge (it contributes no pairs and would silently become the
    /// permissive absent case — see [`Self::may_observe`]).
    pub fn from_bridges(bridges: impl IntoIterator<Item = (i32, Vec<i64>)>) -> Self {
        let mut map: HashMap<i32, HashSet<i64>> = HashMap::new();
        for (bridge_id, chains) in bridges {
            map.entry(bridge_id).or_default().extend(chains);
        }
        Self::PerBridge(map)
    }

    /// Convenience for tests: groups a flat `(bridge_id, chain_id)` pair
    /// stream. **Cannot express a bridge with no contracts** — a bridge that
    /// contributes no pairs is simply absent from the resulting map (the
    /// permissive case), not present with an empty set (the restrictive case).
    /// Use [`Self::from_bridges`] when that distinction matters (it does for
    /// `server.rs`).
    pub fn from_pairs(pairs: impl IntoIterator<Item = (i32, i64)>) -> Self {
        let mut map: HashMap<i32, HashSet<i64>> = HashMap::new();
        for (bridge_id, chain_id) in pairs {
            map.entry(bridge_id).or_default().insert(chain_id);
        }
        Self::PerBridge(map)
    }

    /// May `bridge_id` observe events on `chain_id`?
    ///
    /// The single membership predicate of the stats layer and of the read-side
    /// unindexed-chain filter. There is deliberately no second method: one
    /// question, one answer, both callers.
    ///
    /// | case | result | why |
    /// |---|---|---|
    /// | `AllIndexed` | `true` | no config to consult; every chain is assumed observable |
    /// | bridge in map, chain **in** its set | `true` | a configured contract there — evidence can arrive |
    /// | bridge in map, chain **not in** its set | `false` | never scanned for this bridge; evidence can never arrive |
    /// | bridge **absent** from map (removed from config) | `true` | a config edit must not reinterpret already-indexed history |
    /// | bridge in map with an **empty** set (declared, no contracts) | `false` | falls out of "not in its set"; a misconfiguration worth surfacing |
    ///
    /// The last two rows are **opposite on purpose** and the distinction is
    /// load-bearing on both sides. *Absent* is a decommission: returning `false`
    /// would flip the removed bridge's entire unconfirmed backlog to countable
    /// the moment it leaves the config (one config edit committing an
    /// arbitrary amount of partial data into append-only aggregates), and on
    /// the read side it would hide its complete historical rows.
    /// *Present-but-empty* is a bridge you declared with no contracts — it
    /// genuinely observes nothing, and the startup warn in `server.rs` exists
    /// to make that visible. Do not "simplify" these two into one branch.
    // `map_or(true, ..)` is deliberate here, not `is_some_and(..)`: a bridge
    // absent from the map (removed from config, or never configured) is the
    // permissive default. Flipping this to `false` would retroactively commit
    // that bridge's entire unconfirmed backlog the moment a config edit
    // removes it. Clippy's `unnecessary_map_or` suggestion (`is_none_or`) is
    // semantically identical; `map_or` is kept to keep the `true` default
    // visually adjacent to the comment explaining it.
    #[allow(clippy::unnecessary_map_or)]
    pub fn may_observe(&self, bridge_id: i32, chain_id: i64) -> bool {
        match self {
            IndexedChains::AllIndexed => true,
            IndexedChains::PerBridge(map) => map
                .get(&bridge_id)
                .map_or(true, |chains| chains.contains(&chain_id)),
        }
    }

    /// Number of bridges present in the map (0 for `AllIndexed`).
    pub fn bridge_count(&self) -> usize {
        match self {
            IndexedChains::AllIndexed => 0,
            IndexedChains::PerBridge(map) => map.len(),
        }
    }

    /// Total `(bridge_id, chain_id)` pairs across all bridges (0 for `AllIndexed`).
    pub fn pair_count(&self) -> usize {
        match self {
            IndexedChains::AllIndexed => 0,
            IndexedChains::PerBridge(map) => map.values().map(HashSet::len).sum(),
        }
    }

    /// True only for `PerBridge` with zero pairs (includes an empty map and a
    /// map whose bridges all have empty chain sets).
    pub fn is_empty_config(&self) -> bool {
        matches!(self, IndexedChains::PerBridge(_)) && self.pair_count() == 0
    }

    /// `(bridge_id, chain_count)` for every bridge in the map, for startup
    /// logging. Yields contract-less bridges too (`chain_count == 0`) — that is
    /// the only way a caller can distinguish present-but-empty from absent.
    pub fn bridge_chain_counts(&self) -> Vec<(i32, usize)> {
        match self {
            IndexedChains::AllIndexed => Vec::new(),
            IndexedChains::PerBridge(map) => {
                map.iter().map(|(&b, chains)| (b, chains.len())).collect()
            }
        }
    }

    /// Publishes `STATS_INDEXED_CHAINS` for every bridge in the map. Startup-only.
    pub fn record_metrics(&self) {
        for (bridge_id, chain_count) in self.bridge_chain_counts() {
            super::metrics::STATS_INDEXED_CHAINS
                .with_label_values(&[&bridge_id.to_string()])
                .set(chain_count as f64);
        }
    }

    /// Per-bridge pairs for the read predicate, sorted by bridge id then chain id so
    /// the rendered SQL is deterministic.
    ///
    /// Every bridge **present** in the map contributes an entry, including one whose
    /// chain set is empty — `(b, vec![])`. Dropping such an entry would silently
    /// promote that bridge to the permissive "absent" case, which is the opposite
    /// treatment (see [`IndexedChains::may_observe`]).
    ///
    /// `None` means "no restriction" (`AllIndexed`). `Some(vec![])` means "no bridge
    /// is configured", which under the permissive absent-bridge rule restricts
    /// **nothing** — defensive only; the startup guard rejects an empty config.
    /// `restrict_to` prunes the disjunction to the request's `bridge_ids`; that is a
    /// pure size optimisation because the caller separately ANDs
    /// `bridge_id IN (restrict_to)` — but the pruned ids must also leave the
    /// permissive arm's `NOT IN` list, or a pruned bridge would be treated as absent.
    pub fn configured_pairs(&self, restrict_to: Option<&[i32]>) -> Option<Vec<(i32, Vec<i64>)>> {
        let map = match self {
            IndexedChains::AllIndexed => return None,
            IndexedChains::PerBridge(map) => map,
        };

        let mut pairs: Vec<(i32, Vec<i64>)> = map
            .iter()
            .filter(|(bridge_id, _)| restrict_to.is_none_or(|allowed| allowed.contains(bridge_id)))
            .map(|(&bridge_id, chains)| {
                let mut chains: Vec<i64> = chains.iter().copied().collect();
                chains.sort_unstable();
                (bridge_id, chains)
            })
            .collect();
        pairs.sort_by_key(|(bridge_id, _)| *bridge_id);
        Some(pairs)
    }

    /// Sorted chain ids actually configured for `bridge_id` — the concrete
    /// set, not the permissive [`Self::may_observe`] predicate.
    ///
    /// Returns an empty vec both when `bridge_id` is absent from the map
    /// (removed from config, or never configured) and under `AllIndexed` (no
    /// per-bridge config at all). This is the opposite default from
    /// `may_observe`, and deliberately so: `may_observe` must stay permissive
    /// for an absent bridge so existing history keeps counting, but a
    /// directory listing (`GetBridges`) must report what is actually
    /// configured today, not "assume everything". A present-but-empty bridge
    /// (declared with zero contracts) also yields an empty vec here, which is
    /// the correct answer for that case on both predicates.
    ///
    /// Only for directory views keyed by a single bridge id (`GetBridges`).
    /// Use [`Self::configured_pairs`] instead when iterating every bridge at
    /// once, so the two never derive membership from separate code paths.
    pub fn chain_ids_for(&self, bridge_id: i32) -> Vec<i64> {
        match self {
            IndexedChains::AllIndexed => Vec::new(),
            IndexedChains::PerBridge(map) => {
                let mut chains: Vec<i64> =
                    map.get(&bridge_id).into_iter().flatten().copied().collect();
                chains.sort_unstable();
                chains
            }
        }
    }

    /// Union of every bridge's chain set, deduplicated and sorted ascending.
    /// `None` means "no restriction" (`AllIndexed`); so does `Some(vec![])`, which
    /// can only arise from a config with no bridges at all — see the renderer note
    /// in item 8 and `coding-task-2b.md` item 2.
    ///
    /// Only for the chain-*directory* views (`GetChains`, `/stats/chains`) and the
    /// per-asset token list, which are keyed by chain alone and carry no bridge
    /// linkage. Never use it where a bridge is available.
    pub fn configured_union(&self) -> Option<Vec<i64>> {
        let map = match self {
            IndexedChains::AllIndexed => return None,
            IndexedChains::PerBridge(map) => map,
        };

        let mut union: Vec<i64> = map
            .values()
            .flat_map(|chains| chains.iter().copied())
            .collect::<HashSet<i64>>()
            .into_iter()
            .collect();
        union.sort_unstable();
        Some(union)
    }

    /// Chains configured by two or more DISTINCT bridge ids, as
    /// `(chain_id, sorted bridge ids)`, sorted by `chain_id`. Empty under
    /// `AllIndexed` (no per-bridge configuration to inspect, so a structural
    /// warning would be fabricated). Because each bridge's chains are a
    /// `HashSet`, several contract versions of one bridge on one chain cannot
    /// fabricate an overlap: the bridge id is only counted once regardless of
    /// how many of its contracts sit on that chain.
    pub fn configured_overlaps(&self) -> Vec<(i64, Vec<i32>)> {
        let map = match self {
            IndexedChains::AllIndexed => return Vec::new(),
            IndexedChains::PerBridge(map) => map,
        };

        let mut bridges_by_chain: HashMap<i64, BTreeSet<i32>> = HashMap::new();
        for (&bridge_id, chains) in map {
            for &chain_id in chains {
                bridges_by_chain
                    .entry(chain_id)
                    .or_default()
                    .insert(bridge_id);
            }
        }

        let mut overlaps: Vec<(i64, Vec<i32>)> = bridges_by_chain
            .into_iter()
            .filter(|(_, bridge_ids)| bridge_ids.len() >= 2)
            .map(|(chain_id, bridge_ids)| (chain_id, bridge_ids.into_iter().collect()))
            .collect();
        overlaps.sort_by_key(|(chain_id, _)| *chain_id);
        overlaps
    }

    /// `has_unindexed_chain` for a message. An unknown (NULL) destination always
    /// counts as unindexed. Built on `may_observe`, the same method stats projection
    /// uses, so the flag and the eligibility rule cannot disagree.
    pub fn message_has_unindexed(&self, bridge_id: i32, src: i64, dst: Option<i64>) -> bool {
        !(self.may_observe(bridge_id, src) && dst.is_some_and(|d| self.may_observe(bridge_id, d)))
    }

    /// `has_unindexed_chain` for a transfer. Both token chain columns are NOT NULL.
    pub fn transfer_has_unindexed(&self, bridge_id: i32, src: i64, dst: i64) -> bool {
        !(self.may_observe(bridge_id, src) && self.may_observe(bridge_id, dst))
    }
}

/// `NOT indexed(bridge_col, chain_col)`: true only when the chain id is known,
/// its bridge is present in the current config, and that bridge has no
/// configured contract on that chain.
///
/// This is `IndexedChains::may_observe`'s truth table pushed into SQL — check
/// each rendering rule below against the doc table on [`IndexedChains::may_observe`].
pub(crate) fn chain_unindexed_condition(
    indexed: &IndexedChains,
    bridge_col: impl IntoColumnRef + Clone,
    chain_col: impl IntoColumnRef + Clone,
) -> Condition {
    let map = match indexed {
        // Nothing is ever unindexed: every chain is assumed observable.
        IndexedChains::AllIndexed => return Condition::all().add(Expr::value(false)),
        IndexedChains::PerBridge(map) => map,
    };

    if map.is_empty() {
        // Degenerate "every bridge is absent" case: the permissive default
        // already implies this, so nothing is unindexed either. Fails open in
        // the safe direction — see the startup guard in `server.rs`.
        return Condition::all().add(Expr::value(false));
    }

    // Enumerate one disjunct per bridge *present in the map*. A bridge absent
    // from the map contributes no disjunct at all, so the whole condition is
    // `FALSE` for its rows (not unindexed -> defer). That is the permissive
    // default and needs no special-casing here — do not add a catch-all arm
    // for "bridge not in map" below, it would invert the intended asymmetry.
    let mut any = Condition::any();
    for (&bridge_id, chains) in map {
        if chains.is_empty() {
            // Present with an empty set: every chain is unindexed for it.
            any = any.add(Expr::col(bridge_col.clone()).eq(bridge_id));
        } else {
            let chain_list: Vec<i64> = chains.iter().copied().collect();
            any = any.add(
                Condition::all()
                    .add(Expr::col(bridge_col.clone()).eq(bridge_id))
                    .add(Expr::col(chain_col.clone()).is_not_in(chain_list)),
            );
        }
    }

    // Explicit `IS NOT NULL` guard: a NULL chain id must defer unconditionally.
    // `NULL NOT IN (...)` would already evaluate to NULL (filtered out), but
    // that invariant must be readable in the code, not incidental to
    // three-valued SQL logic.
    Condition::all()
        .add(Expr::col(chain_col.clone()).is_not_null())
        .add(any)
}

/// Parent-message side of eligibility: the message is confirmed, or its
/// destination confirmation can never arrive.
pub(crate) fn message_countable_condition(indexed: &IndexedChains) -> Condition {
    Condition::any()
        .add(super::projection::finalized_message_stats_condition())
        .add(chain_unindexed_condition(
            indexed,
            (
                crosschain_messages::Entity,
                crosschain_messages::Column::BridgeId,
            ),
            (
                crosschain_messages::Entity,
                crosschain_messages::Column::DstChainId,
            ),
        ))
}

/// Transfer side: every unknown token endpoint sits on a chain this bridge
/// does not index, and at least one endpoint is known.
pub(crate) fn transfer_identity_ready_condition(indexed: &IndexedChains) -> Condition {
    let bridge_col = (
        crosschain_transfers::Entity,
        crosschain_transfers::Column::BridgeId,
    );
    let src_addr_col = (
        crosschain_transfers::Entity,
        crosschain_transfers::Column::TokenSrcAddress,
    );
    let dst_addr_col = (
        crosschain_transfers::Entity,
        crosschain_transfers::Column::TokenDstAddress,
    );

    let src_ready = Condition::any()
        .add(Expr::col(src_addr_col).is_not_null())
        .add(chain_unindexed_condition(
            indexed,
            bridge_col,
            (
                crosschain_transfers::Entity,
                crosschain_transfers::Column::TokenSrcChainId,
            ),
        ));
    let dst_ready = Condition::any()
        .add(Expr::col(dst_addr_col).is_not_null())
        .add(chain_unindexed_condition(
            indexed,
            bridge_col,
            (
                crosschain_transfers::Entity,
                crosschain_transfers::Column::TokenDstChainId,
            ),
        ));
    let at_least_one_known = Condition::any()
        .add(Expr::col(src_addr_col).is_not_null())
        .add(Expr::col(dst_addr_col).is_not_null());

    Condition::all()
        .add(src_ready)
        .add(dst_ready)
        .add(at_least_one_known)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DatabaseBackend, EntityTrait, QueryFilter, QueryTrait};

    /// Renders a condition's WHERE clause with values inlined, following the
    /// pattern in `filters.rs`'s own condition-rendering tests.
    fn render(cond: Condition) -> String {
        crosschain_messages::Entity::find()
            .filter(cond)
            .build(DatabaseBackend::Postgres)
            .to_string()
    }

    // --- may_observe: one test per row of the truth table ---

    #[test]
    fn test_may_observe_all_indexed_observes_everything() {
        let indexed = IndexedChains::AllIndexed;
        assert!(indexed.may_observe(1, 100));
        assert!(indexed.may_observe(999, -1));
    }

    #[test]
    fn test_may_observe_known_bridge_chain_in_set_is_true() {
        let indexed = IndexedChains::from_pairs([(1, 100), (1, 200)]);
        assert!(indexed.may_observe(1, 100));
    }

    #[test]
    fn test_may_observe_known_bridge_chain_outside_set_is_false() {
        let indexed = IndexedChains::from_pairs([(1, 100), (1, 200)]);
        assert!(!indexed.may_observe(1, 999));
    }

    #[test]
    fn test_may_observe_absent_bridge_is_permissive() {
        let indexed = IndexedChains::from_pairs([(1, 100)]);
        // bridge 2 never appears in the map: permissive default.
        assert!(indexed.may_observe(2, 100));
        assert!(indexed.may_observe(2, 999));
    }

    #[test]
    fn test_may_observe_absent_vs_present_but_empty_are_opposite() {
        // Bridge 1 is absent from the map entirely.
        let absent = IndexedChains::from_pairs([(2, 100)]);
        assert!(absent.may_observe(1, 100));

        // Bridge 1 is present but declared with zero contracts.
        let present_but_empty = IndexedChains::from_bridges([(1, vec![]), (2, vec![100])]);
        assert!(!present_but_empty.may_observe(1, 100));
        assert!(!present_but_empty.may_observe(1, 999));
    }

    #[test]
    fn test_from_pairs_deduplicates() {
        let indexed = IndexedChains::from_pairs([(1, 100), (1, 100), (1, 200)]);
        assert_eq!(indexed.pair_count(), 2);
    }

    #[test]
    fn test_from_bridges_contract_less_bridge_present_with_empty_set() {
        let indexed = IndexedChains::from_bridges([(1, vec![]), (2, vec![100, 200])]);
        assert_eq!(indexed.bridge_count(), 2);
        assert_eq!(indexed.pair_count(), 2);
        // Bridge 1 is present (declared) but with an empty set: restrictive.
        assert!(!indexed.may_observe(1, 100));

        // from_pairs cannot express this: a contract-less bridge contributes
        // no pairs, so it is simply absent (permissive), not present-but-empty.
        let from_pairs = IndexedChains::from_pairs([(2, 100), (2, 200)]);
        assert!(from_pairs.may_observe(1, 100));
    }

    // --- chain_unindexed_condition: rendered-SQL pinning ---

    fn cols() -> (
        (crosschain_messages::Entity, crosschain_messages::Column),
        (crosschain_messages::Entity, crosschain_messages::Column),
    ) {
        (
            (
                crosschain_messages::Entity,
                crosschain_messages::Column::BridgeId,
            ),
            (
                crosschain_messages::Entity,
                crosschain_messages::Column::DstChainId,
            ),
        )
    }

    #[test]
    fn test_chain_unindexed_condition_all_indexed_renders_false() {
        let (bridge_col, chain_col) = cols();
        let sql = render(chain_unindexed_condition(
            &IndexedChains::AllIndexed,
            bridge_col,
            chain_col,
        ));
        assert!(sql.contains("WHERE FALSE"), "sql was: {sql}");
    }

    #[test]
    fn test_chain_unindexed_condition_zero_bridges_renders_false() {
        let (bridge_col, chain_col) = cols();
        let sql = render(chain_unindexed_condition(
            &IndexedChains::from_pairs(std::iter::empty()),
            bridge_col,
            chain_col,
        ));
        assert!(sql.contains("WHERE FALSE"), "sql was: {sql}");
    }

    #[test]
    fn test_chain_unindexed_condition_absent_bridge_renders_no_disjunct() {
        let (bridge_col, chain_col) = cols();
        // Only bridge 2 is in the map; querying with a condition that only
        // ever mentions bridge 1's rows still must not classify them as
        // unindexed. We assert this indirectly: the disjunction only contains
        // a clause for bridge 2, never for bridge 1.
        let sql = render(chain_unindexed_condition(
            &IndexedChains::from_pairs([(2, 100)]),
            bridge_col,
            chain_col,
        ));
        assert!(!sql.contains("\"bridge_id\" = 1"), "sql was: {sql}");
        assert!(sql.contains("\"bridge_id\" = 2"), "sql was: {sql}");
    }

    #[test]
    fn test_chain_unindexed_condition_present_but_empty_renders_bare_bridge_eq() {
        let (bridge_col, chain_col) = cols();
        let sql = render(chain_unindexed_condition(
            &IndexedChains::from_bridges([(1, vec![])]),
            bridge_col,
            chain_col,
        ));
        assert!(sql.contains("IS NOT NULL"), "sql was: {sql}");
        assert!(sql.contains("\"bridge_id\" = 1"), "sql was: {sql}");
        assert!(!sql.contains("NOT IN"), "sql was: {sql}");
    }

    #[test]
    fn test_chain_unindexed_condition_per_bridge_renders_not_in() {
        let (bridge_col, chain_col) = cols();
        let sql = render(chain_unindexed_condition(
            &IndexedChains::from_pairs([(1, 100), (1, 200)]),
            bridge_col,
            chain_col,
        ));
        assert!(sql.contains("IS NOT NULL"), "sql was: {sql}");
        assert!(sql.contains("\"bridge_id\" = 1"), "sql was: {sql}");
        assert!(sql.contains("NOT IN"), "sql was: {sql}");
    }

    // --- configured_pairs ---

    #[test]
    fn test_configured_pairs_all_indexed_is_none() {
        assert_eq!(IndexedChains::AllIndexed.configured_pairs(None), None);
    }

    #[test]
    fn test_configured_pairs_no_bridges_is_some_empty() {
        let indexed = IndexedChains::from_pairs(std::iter::empty());
        assert_eq!(indexed.configured_pairs(None), Some(Vec::new()));
    }

    #[test]
    fn test_configured_pairs_present_but_empty_bridge_is_not_dropped() {
        let indexed = IndexedChains::from_bridges([(1, vec![]), (2, vec![200, 100])]);
        assert_eq!(
            indexed.configured_pairs(None),
            Some(vec![(1, vec![]), (2, vec![100, 200])])
        );
    }

    #[test]
    fn test_configured_pairs_sorted_by_bridge_then_chain() {
        let indexed = IndexedChains::from_pairs([(2, 300), (1, 200), (2, 100), (1, 100)]);
        assert_eq!(
            indexed.configured_pairs(None),
            Some(vec![(1, vec![100, 200]), (2, vec![100, 300])])
        );
    }

    #[test]
    fn test_configured_pairs_restrict_to_keeps_only_listed_bridges() {
        let indexed = IndexedChains::from_pairs([(1, 100), (2, 250), (3, 999)]);
        assert_eq!(
            indexed.configured_pairs(Some(&[2])),
            Some(vec![(2, vec![250])])
        );
    }

    #[test]
    fn test_configured_pairs_restrict_to_none_keeps_all() {
        let indexed = IndexedChains::from_pairs([(1, 100), (2, 250)]);
        assert_eq!(
            indexed.configured_pairs(None),
            Some(vec![(1, vec![100]), (2, vec![250])])
        );
    }

    // --- chain_ids_for ---

    #[test]
    fn test_chain_ids_for_all_indexed_is_empty() {
        assert_eq!(
            IndexedChains::AllIndexed.chain_ids_for(1),
            Vec::<i64>::new()
        );
    }

    #[test]
    fn test_chain_ids_for_absent_bridge_is_empty() {
        // Bridge 1 is absent from the map (removed from config, or never
        // configured): the concrete-set accessor reports nothing, unlike the
        // permissive `may_observe`.
        let indexed = IndexedChains::from_pairs([(2, 100)]);
        assert_eq!(indexed.chain_ids_for(1), Vec::<i64>::new());
    }

    #[test]
    fn test_chain_ids_for_present_but_empty_bridge_is_empty() {
        let indexed = IndexedChains::from_bridges([(1, vec![]), (2, vec![100])]);
        assert_eq!(indexed.chain_ids_for(1), Vec::<i64>::new());
    }

    #[test]
    fn test_chain_ids_for_multi_chain_bridge_is_sorted() {
        let indexed = IndexedChains::from_pairs([(1, 300), (1, 100), (1, 200), (1, 100)]);
        assert_eq!(indexed.chain_ids_for(1), vec![100, 200, 300]);
    }

    // --- configured_union ---

    #[test]
    fn test_configured_union_all_indexed_is_none() {
        assert_eq!(IndexedChains::AllIndexed.configured_union(), None);
    }

    #[test]
    fn test_configured_union_deduplicated_and_sorted() {
        let indexed = IndexedChains::from_pairs([(1, 250), (1, 100), (2, 100), (2, 1)]);
        assert_eq!(indexed.configured_union(), Some(vec![1, 100, 250]));
    }

    #[test]
    fn test_configured_union_no_bridges_is_some_empty() {
        let indexed = IndexedChains::from_pairs(std::iter::empty());
        assert_eq!(indexed.configured_union(), Some(Vec::new()));
    }

    // --- flag helpers ---

    #[test]
    fn test_message_has_unindexed_both_endpoints_in_set_is_false() {
        let indexed = IndexedChains::from_pairs([(1, 1), (1, 100)]);
        assert!(!indexed.message_has_unindexed(1, 1, Some(100)));
    }

    #[test]
    fn test_message_has_unindexed_src_out_is_true() {
        let indexed = IndexedChains::from_pairs([(1, 100)]);
        assert!(indexed.message_has_unindexed(1, 1, Some(100)));
    }

    #[test]
    fn test_message_has_unindexed_dst_out_is_true() {
        let indexed = IndexedChains::from_pairs([(1, 1)]);
        assert!(indexed.message_has_unindexed(1, 1, Some(100)));
    }

    #[test]
    fn test_message_has_unindexed_null_dst_is_true_under_all_indexed_and_per_bridge() {
        assert!(IndexedChains::AllIndexed.message_has_unindexed(1, 1, None));
        let indexed = IndexedChains::from_pairs([(1, 1)]);
        assert!(indexed.message_has_unindexed(1, 1, None));
    }

    #[test]
    fn test_transfer_has_unindexed_both_endpoints_in_set_is_false() {
        let indexed = IndexedChains::from_pairs([(1, 1), (1, 100)]);
        assert!(!indexed.transfer_has_unindexed(1, 1, 100));
    }

    #[test]
    fn test_transfer_has_unindexed_src_out_is_true() {
        let indexed = IndexedChains::from_pairs([(1, 100)]);
        assert!(indexed.transfer_has_unindexed(1, 1, 100));
    }

    #[test]
    fn test_transfer_has_unindexed_dst_out_is_true() {
        let indexed = IndexedChains::from_pairs([(1, 1)]);
        assert!(indexed.transfer_has_unindexed(1, 1, 100));
    }

    // --- configured_overlaps ---

    #[test]
    fn test_configured_overlaps_all_indexed_is_empty() {
        assert_eq!(IndexedChains::AllIndexed.configured_overlaps(), Vec::new());
    }

    #[test]
    fn test_configured_overlaps_one_bridge_multiple_contract_versions_no_overlap() {
        // Bridge 1 has two contract versions on chain 100 — represented as one
        // bridge with chain 100 already deduplicated into its `HashSet`, so
        // this cannot fabricate an overlap.
        let indexed = IndexedChains::from_pairs([(1, 100), (1, 100)]);
        assert_eq!(indexed.configured_overlaps(), Vec::new());
    }

    #[test]
    fn test_configured_overlaps_two_distinct_bridges_sharing_a_chain() {
        let indexed = IndexedChains::from_pairs([(2, 100), (1, 100), (1, 200)]);
        assert_eq!(indexed.configured_overlaps(), vec![(100, vec![1, 2])]);
    }

    #[test]
    fn test_configured_overlaps_disabled_bridges_still_counted() {
        // `IndexedChains` construction is independent of `enabled`; a caller
        // that includes disabled bridges (as `server.rs` does) still sees them
        // participate in an overlap.
        let indexed = IndexedChains::from_pairs([(1, 100), (2, 100)]);
        assert_eq!(indexed.configured_overlaps(), vec![(100, vec![1, 2])]);
    }

    #[test]
    fn test_configured_overlaps_sorted_by_chain_id() {
        let indexed = IndexedChains::from_pairs([(1, 300), (2, 300), (1, 100), (2, 100)]);
        assert_eq!(
            indexed.configured_overlaps(),
            vec![(100, vec![1, 2]), (300, vec![1, 2])]
        );
    }

    /// The asymmetry a future reader is most likely to "simplify" away: an absent
    /// bridge with a real destination is unflagged, while a present-but-empty
    /// bridge is flagged, for the same (src, dst) pair.
    #[test]
    fn test_message_has_unindexed_absent_vs_present_but_empty_bridge_are_opposite() {
        // Bridge 1 is absent from the map entirely: permissive, unflagged.
        let absent = IndexedChains::from_pairs([(2, 1)]);
        assert!(!absent.message_has_unindexed(1, 1, Some(100)));

        // Bridge 1 is present but declared with zero contracts: restrictive, flagged.
        let present_but_empty = IndexedChains::from_bridges([(1, vec![]), (2, vec![1])]);
        assert!(present_but_empty.message_has_unindexed(1, 1, Some(100)));
    }
}
