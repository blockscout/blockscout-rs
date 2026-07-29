// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{IntCounterVec, register_int_counter_vec};

// Avalanche-specific metrics. Keep the `outcome` label to the closed set
// documented below so cardinality stays bounded (`bridges × ~9`).
lazy_static! {
    /// ICTT payload classification outcomes per bridge, one increment per
    /// consolidation attempt — except `skipped_disabled`, which is
    /// incremented once per suppressed log at ingestion time
    /// (`avalanche/mod.rs`), because consolidation is not config-aware.
    ///
    /// `outcome` is a closed set: `reconstructed`, `skipped_multi_hop`,
    /// `skipped_register_remote`, `skipped_disabled`,
    /// `skipped_no_destination_event`, `skipped_no_payload_source`,
    /// `rejected_decode`, `variant_mismatch`, `no_credit_expected`. Do not
    /// grow this to a free-form reason string.
    pub static ref AVALANCHE_ICTT_PAYLOAD_OUTCOMES_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_avalanche_ictt_payload_outcomes_total",
        "ICTT payload classification outcomes per bridge, one increment per consolidation attempt",
        &["bridge_id", "outcome"],
    )
    .unwrap();
}
