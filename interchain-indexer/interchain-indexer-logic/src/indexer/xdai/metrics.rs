// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{GaugeVec, register_gauge_vec};

// xDai-specific metrics. Keep labels low-cardinality: bridge_id.
lazy_static! {
    /// Size of the Gno→Eth `pending_message_hash_events` correlation queue,
    /// per bridge.
    ///
    /// This map holds `SignedForUserRequest` / `CollectedSignatures` events
    /// that arrived before their `UserRequestForSignature` source (catch-up
    /// and realtime scan the same Gnosis chain concurrently, so out-of-order
    /// arrival is real even though the correlation is same-chain). It has no
    /// TTL, no cap and no offload, so its occupancy is proportional to how
    /// far realtime has run ahead of catch-up on the source stream. Mirrors
    /// `indexer::metrics::AMB_PENDING_CORRELATION_QUEUE`.
    ///
    /// Set from the xDai event handlers on every insert and drain, which is
    /// where the only mutations happen — not from a request handler.
    pub static ref XDAI_PENDING_CORRELATION_QUEUE: GaugeVec = register_gauge_vec!(
        "interchain_indexer_xdai_pending_correlation_queue",
        "queued xDai Gno->Eth events awaiting their UserRequestForSignature source",
        &["bridge_id"],
    )
    .unwrap();
}
