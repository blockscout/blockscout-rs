// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{
    GaugeVec, Histogram, IntCounter, IntCounterVec, register_gauge_vec, register_histogram,
    register_int_counter, register_int_counter_vec,
};

// Metrics for stats projection eligibility. Keep labels low-cardinality: never
// label by chain id, asset id, or transfer id.
lazy_static! {
    /// Size of the per-bridge indexed-chain set, set once at startup.
    pub static ref STATS_INDEXED_CHAINS: GaugeVec = register_gauge_vec!(
        "interchain_indexer_stats_indexed_chains",
        "chains with a configured contract per bridge, as seen by stats eligibility",
        &["bridge_id"],
    )
    .unwrap();

    /// Deferral EVENTS (not distinct rows): a row is re-evaluated whenever its
    /// canonical key is flushed again, so repeated deferral increments repeatedly.
    /// `reason` is `identity_incomplete` (a token endpoint is missing and its
    /// chain is still indexed for the bridge) or `awaiting_confirmation` (the
    /// parent message is not yet finalized and its destination chain is still
    /// indexed for the bridge).
    pub static ref STATS_TRANSFERS_DEFERRED_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_stats_transfers_deferred_total",
        "transfer stats deferral events by reason (events, not distinct rows): \
         identity_incomplete (missing token endpoint, chain still indexed) or \
         awaiting_confirmation (parent message unconfirmed, destination chain still indexed)",
        &["reason"],
    )
    .unwrap();

    /// Stats asset union-find merges by outcome: `merged` or
    /// `refused_chain_collision` (both components hold a token on the same
    /// chain, the only unresolvable conflict).
    pub static ref STATS_ASSET_MERGES_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_stats_asset_merges_total",
        "stats asset component merges by outcome",
        &["outcome"],
    )
    .unwrap();

    /// `crosschain_transfers` rows repointed per successful asset merge.
    pub static ref STATS_ASSET_MERGE_REPOINTED_TRANSFERS: Histogram = register_histogram!(
        "interchain_indexer_stats_asset_merge_repointed_transfers",
        "crosschain_transfers rows repointed per asset merge",
        vec![1.0, 10.0, 100.0, 1_000.0, 10_000.0, 100_000.0, 1_000_000.0]
    )
    .unwrap();

    /// `stats_asset_edges` folds that summed two rows with different
    /// `amount_side` (the winner's side is kept; the result is approximate).
    pub static ref STATS_EDGE_MIXED_AMOUNT_SIDE_TOTAL: IntCounter = register_int_counter!(
        "interchain_indexer_stats_edge_mixed_amount_side_total",
        "stats_asset_edges folds that summed two different amount sides (approximate result)"
    )
    .unwrap();

    /// `stats_asset_edges` folds by how the loser's `cumulative_amount` was
    /// converted before being added to the winner's: `scaled_up` /
    /// `scaled_down` (both decimals known and different), or
    /// `unscaled_unknown_decimals` / `unscaled_overflow` (added as-is).
    pub static ref STATS_EDGE_RESCALED_FOLD_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_stats_edge_rescaled_fold_total",
        "stats_asset_edges folds by how the loser amount was converted",
        &["mode"],
    )
    .unwrap();

    /// Transfers skipped on the non-merge counting path because a token's
    /// decimals changed for an existing edge. The transfer is still marked
    /// processed and the transaction still commits (task Decision 7).
    pub static ref STATS_EDGE_DECIMALS_CONFLICT_TOTAL: IntCounter = register_int_counter!(
        "interchain_indexer_stats_edge_decimals_conflict_total",
        "transfers skipped because a token's decimals changed for an existing edge (non-merge path)"
    )
    .unwrap();

    /// Actual cross-bridge duplicate-user impact of the last successful
    /// `stats_chains` recomputation: `SUM` over chains of
    /// `SUM(per_bridge_count) - global_distinct_count`, i.e. the number of
    /// extra address contributions a full bridge-sum would add. `kind` is the
    /// only label — never chain id or bridge set. This is an upper bound
    /// across *all* bridges, not the overcount any one multi-bridge request
    /// would see; a `GetChainsStats` request only ever selects a subset.
    pub static ref STATS_CHAINS_BRIDGE_SUM_OVERCOUNT_USERS: GaugeVec = register_gauge_vec!(
        "interchain_indexer_stats_chains_bridge_sum_overcount_users",
        "extra address contributions a full per-bridge stats_chains sum would add over the exact \
         global count, by kind (transfer|message); an all-bridge upper bound, not per-request",
        &["kind"],
    )
    .unwrap();

    /// Number of chains with a positive overcount delta for `kind`, from the
    /// same recomputation as [`STATS_CHAINS_BRIDGE_SUM_OVERCOUNT_USERS`].
    pub static ref STATS_CHAINS_BRIDGE_SUM_AFFECTED_CHAINS: GaugeVec = register_gauge_vec!(
        "interchain_indexer_stats_chains_bridge_sum_affected_chains",
        "chains with a positive bridge-sum overcount delta, by kind (transfer|message)",
        &["kind"],
    )
    .unwrap();
}
