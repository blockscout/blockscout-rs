// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{IntCounterVec, register_int_counter_vec};

// AMB-specific metrics. Keep labels low-cardinality: `bridge_id`, and
// `chain_id` where the bridge alone does not identify the misconfiguration.
lazy_static! {
    /// AMB `messageId` collisions observed at consolidation: two different
    /// message bodies sharing a structured `messageId`. Incremented once per
    /// displaced body (the source body that lost the canonical slot, plus any
    /// conflicting second destination executions).
    pub static ref AMB_IDENTITY_CONFLICTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_amb_identity_conflicts_total",
        "AMB messageId collisions observed at consolidation, per displaced body",
        &["bridge_id"],
    )
    .unwrap();

    /// Logs dropped because the contract version in force at the log's block
    /// does not declare its `topic0`, while another configured version of the
    /// same address does.
    ///
    /// Zero is the expected value. Anything else means a `started_at_block`
    /// boundary in `bridges.json` disagrees with the chain: real events are
    /// being discarded, and — unlike a fetch failure — nothing else reports it,
    /// because the blocks *were* scanned and the ledger has no row.
    ///
    /// Labelled by `bridge_id` **and** `chain_id`: the offending boundary lives
    /// in one bridge's contract entry, and several bridges can be configured on
    /// the same chain, so `chain_id` alone does not point at the config to fix.
    pub static ref AMB_LOGS_DROPPED_WRONG_VERSION_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_amb_logs_dropped_wrong_version_total",
        "AMB logs dropped because the version active at their block does not declare their topic0",
        &["bridge_id", "chain_id"],
    )
    .unwrap();
}
