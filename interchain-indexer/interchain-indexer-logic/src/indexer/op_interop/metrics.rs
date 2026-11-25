// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{IntCounterVec, register_int_counter_vec};

// OP interop metrics. Keep labels low-cardinality: bridge_id, chain_id and a
// closed set of reasons. Wrong-version drops are not here: they use the shared
// `crate::indexer::metrics::LOGS_DROPPED_WRONG_VERSION_TOTAL` with
// `protocol = "op_interop"`.
lazy_static! {
    /// Interop logs decoded, by event. `event` is a closed set:
    /// `sent_message`, `executing_message`, `relayed_message`, `send_eth`,
    /// `relay_eth`.
    ///
    /// Counted per processing attempt, so a retried or rescanned block counts
    /// again. On one chain, `relayed_message` should grow together with
    /// `raw_executions_total{origin="messenger"}`; a persistent gap means the
    /// inbox event changed shape.
    pub static ref OP_INTEROP_EVENTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_op_interop_events_total",
        "OP interop logs decoded, by event",
        &["bridge_id", "chain_id", "event"],
    )
    .unwrap();

    /// Messages not indexed because their other endpoint is outside this
    /// bridge. `reason` is a closed set: `destination_not_configured`,
    /// `destination_out_of_range`, `source_not_configured`,
    /// `source_out_of_range`. A skip creates no buffer entry and is not an
    /// error.
    pub static ref OP_INTEROP_SKIPPED_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_op_interop_skipped_total",
        "OP interop messages skipped because the other chain is not part of the bridge",
        &["bridge_id", "chain_id", "reason"],
    )
    .unwrap();

    /// Every `CrossL2Inbox` `ExecutingMessage` indexed as a ledger row, by the
    /// origin in its identifier. `origin` is a closed set: `messenger` (the
    /// messenger's own executions and pull reads of messenger logs) and
    /// `foreign` (reads of any other contract's log). Counted per processing
    /// attempt, so a retried or rescanned block counts again; a failed write
    /// still counts (write failures show in the failure-ledger metrics). Per
    /// chain, `messenger` minus the relays that found a candidate
    /// (`events_total{event="relayed_message"}` minus
    /// `relays_unpaired_total`) is the number of messenger-origin pull reads.
    pub static ref OP_INTEROP_RAW_EXECUTIONS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_op_interop_raw_executions_total",
        "OP interop CrossL2Inbox ExecutingMessage logs indexed as ledger rows, by identifier origin",
        &["bridge_id", "chain_id", "origin"],
    )
    .unwrap();

    /// `RelayedMessage` logs with no candidate in their relay window (no
    /// messenger-origin `ExecutingMessage` of their source chain since the
    /// previous relay). The message still completes; only its
    /// `executing_message` detail is missing. Zero is the expected value.
    pub static ref OP_INTEROP_RELAYS_UNPAIRED_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_op_interop_relays_unpaired_total",
        "OP interop relays indexed without a paired ExecutingMessage",
        &["bridge_id", "chain_id"],
    )
    .unwrap();

    /// kind: unrecognized_payload, send_eth_missing, send_eth_unpaired,
    /// send_eth_mismatch, relay_eth_missing, relay_eth_mismatch. Zero is the
    /// expected value. The two relay kinds are counted per consolidation
    /// attempt; the other four are counted per ingestion of the transaction,
    /// so a retried or rescanned block counts them again.
    pub static ref OP_INTEROP_ETH_TRANSFER_ANOMALIES_TOTAL: IntCounterVec =
        register_int_counter_vec!(
            "interchain_indexer_op_interop_eth_transfer_anomalies_total",
            "OP interop ETH transfer payloads or bridge events that disagree with the expected shape",
            &["bridge_id", "kind"],
        )
        .unwrap();

    /// Observations that contradict what the buffer already holds. `kind` is
    /// a closed set: `duplicate_leg` (a different log for a side that is
    /// already filled; impossible on a confirmed chain), `key_collision` (two
    /// message hashes under one 64-bit buffer key) and `identifier_mismatch`
    /// (a relay has candidates but none points at the indexed source log;
    /// nothing is nested or linked).
    /// The first observation always wins. Zero is the expected value.
    pub static ref OP_INTEROP_LEG_CONFLICTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_op_interop_leg_conflicts_total",
        "OP interop observations that conflict with the buffered message",
        &["bridge_id", "kind"],
    )
    .unwrap();
}
