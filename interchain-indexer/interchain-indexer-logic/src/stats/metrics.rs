// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{GaugeVec, IntCounterVec, register_gauge_vec, register_int_counter_vec};

// Metrics for stats projection eligibility. Keep labels low-cardinality: never
// label by chain id or transfer id.
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
}
