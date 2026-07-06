// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{IntCounterVec, register_int_counter_vec};

// AMB-specific metrics. Keep labels low-cardinality: bridge_id only.
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
}
