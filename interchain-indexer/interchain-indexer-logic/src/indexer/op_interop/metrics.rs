// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{IntCounterVec, register_int_counter_vec};

// OP interop metrics. Keep labels low-cardinality: bridge_id, chain_id and a
// closed set of reasons. Wrong-version drops are not here: they use the shared
// `crate::indexer::metrics::LOGS_DROPPED_WRONG_VERSION_TOTAL` with
// `protocol = "op_interop"`.
lazy_static! {
    /// Interop logs decoded, by event. `event` is a closed set:
    /// `sent_message`, `executing_message`, `relayed_message`.
    ///
    /// Counted per processing attempt, so a retried or rescanned block counts
    /// again. On one chain, `relayed_message` and messenger-origin
    /// `executing_message` should grow together; a persistent gap means the
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

    /// `ExecutingMessage` logs that did not become part of a message.
    /// `reason` is a closed set: `foreign_origin` (the identifier's origin is
    /// not the messenger: someone read the inbox directly) and `unpaired` (a
    /// messenger-origin log no `RelayedMessage` of the transaction consumed).
    pub static ref OP_INTEROP_EXECUTING_MESSAGES_IGNORED_TOTAL: IntCounterVec =
        register_int_counter_vec!(
            "interchain_indexer_op_interop_executing_messages_ignored_total",
            "OP interop ExecutingMessage logs that were not paired with a relay",
            &["bridge_id", "chain_id", "reason"],
        )
        .unwrap();

    /// `RelayedMessage` logs with no usable `ExecutingMessage` before them in
    /// the transaction. The message still completes; only its
    /// `executing_message` detail is missing. Zero is the expected value.
    pub static ref OP_INTEROP_RELAYS_UNPAIRED_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_op_interop_relays_unpaired_total",
        "OP interop relays indexed without a paired ExecutingMessage",
        &["bridge_id", "chain_id"],
    )
    .unwrap();

    /// Observations that contradict what the buffer already holds. `kind` is
    /// a closed set: `duplicate_leg` (a different log for a side that is
    /// already filled; impossible on a confirmed chain), `key_collision` (two
    /// message hashes under one 64-bit buffer key) and `identifier_mismatch`
    /// (a paired `ExecutingMessage` does not point at the indexed source log).
    /// The first observation always wins. Zero is the expected value.
    pub static ref OP_INTEROP_LEG_CONFLICTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_op_interop_leg_conflicts_total",
        "OP interop observations that conflict with the buffered message",
        &["bridge_id", "kind"],
    )
    .unwrap();
}
