// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{GaugeVec, IntGaugeVec, register_gauge_vec, register_int_gauge_vec};

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

    /// `1` when the Foreign proxy's live `erc20token()` disagrees with the
    /// newest configured version's `source_asset`; `0` when they agree.
    ///
    /// Set once, at indexer start, by
    /// `indexer::check_source_asset_matches_latest`. Deliberately left
    /// *unset* when the `eth_call` itself fails: a transient RPC failure is
    /// not evidence either way, and publishing `0` there would assert
    /// agreement that was never observed.
    ///
    /// A mismatch means the bridge was upgraded without a matching
    /// `bridges.json` update, so every Ethereum→Gnosis deposit indexed from
    /// that point carries the wrong `token_src_address` — and because the
    /// asset comes from a static table rather than from a log, nothing
    /// downstream can notice. This gauge plus the accompanying `error` log is
    /// the whole detection path, which is why the condition is not left at
    /// `warn` among ordinary startup warnings. Alert on `> 0`.
    ///
    /// Startup does not fail on a mismatch: `spawn_configured_indexers`
    /// propagates a construction error, so failing here would take the AMB
    /// and Avalanche indexers down with it — a service-wide outage traded for
    /// a labelling error confined to one direction of one bridge.
    pub static ref XDAI_SOURCE_ASSET_MISMATCH: IntGaugeVec = register_int_gauge_vec!(
        "interchain_indexer_xdai_source_asset_mismatch",
        "1 when the xDai Foreign proxy's live erc20token() disagrees with the configured newest source_asset",
        &["bridge_id"],
    )
    .unwrap();
}
