// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{GaugeVec, IntCounterVec, register_gauge_vec, register_int_counter_vec};

// Metrics for the shared failed-range ledger (`FailureLedger`) and the
// `RangeDriver` retry loop. Keep labels low-cardinality: bridge_id and
// chain_id.
lazy_static! {
    /// Number of blocks currently recorded as failed (open, unresolved) in
    /// `indexer_failures`, per bridge and chain. Refreshed only by the
    /// periodic metrics worker (`spawn_indexing_progress_metrics_worker` /
    /// `refresh_failure_ledger_gauges` in `interchain-indexer-server`), never
    /// from a driver loop — this is a periodic snapshot, not a live counter.
    /// The worker is keyed off the configured `(bridge, chain)` targets rather
    /// than driver liveness, so a pair whose indexer never started still gets
    /// a series, and a pair whose last hole was just resolved reports an
    /// explicit `0` instead of a frozen last-known value. A failed totals
    /// query leaves the previous values in place instead of zeroing them.
    pub static ref FAILED_BLOCKS: GaugeVec = register_gauge_vec!(
        "interchain_indexer_failed_blocks",
        "number of blocks currently recorded as failed in indexer_failures",
        &["bridge_id", "chain_id"],
    )
    .unwrap();

    /// Age, in seconds, of the oldest still-open failed interval (its
    /// `created_at`), per bridge and chain. Refreshed by the same metrics
    /// worker as `FAILED_BLOCKS`. This is the sole detector for a retry pass
    /// that has stopped converging: nothing else observes whether recorded
    /// holes are actually shrinking over time, so an alert on this gauge is
    /// required operational scope, not polish. `0` means no open interval for
    /// the pair.
    pub static ref OLDEST_OPEN_HOLE_AGE_SECONDS: GaugeVec = register_gauge_vec!(
        "interchain_indexer_oldest_open_hole_age_seconds",
        "age in seconds of the oldest open failed interval's first_failed_at",
        &["bridge_id", "chain_id"],
    )
    .unwrap();

    /// Failure-ledger writes that actually issued a database statement
    /// (cache-elided `resolve` calls do not count), split by operation.
    /// `operation` is a closed set: `record`, `resolve`.
    pub static ref FAILURE_LEDGER_WRITES_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_failure_ledger_writes_total",
        "failure ledger writes that issued a database statement, by operation",
        &["bridge_id", "operation"],
    )
    .unwrap();

    /// Number of times a `record` write could not be persisted after
    /// exhausting `record_retry_attempts` and the driver escalated (stopped
    /// consuming the stream, indexer state becomes Failed). Labelled by chain
    /// because the chains of one bridge now index independently, so "which
    /// chain escalated" is not answerable from `bridge_id` alone.
    pub static ref FAILURE_RECORD_ESCALATIONS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_failure_record_escalations_total",
        "unrecordable failures that escalated to a fatal indexer error",
        &["bridge_id", "chain_id"],
    )
    .unwrap();

    // Metrics for the `GetIndexingProgress` API.
    // Refreshed only by the periodic metrics worker
    // (`spawn_indexing_progress_metrics_worker` in `server.rs`), never from a
    // request handler — a gauge refreshed from a request handler is frozen
    // between calls, which is worse than not having it.

    /// Scanned share of the historical block range, 0-100, per bridge and chain.
    /// This is the scanned share only, never a completeness measure: it can read
    /// 100 while `interchain_indexer_failed_blocks` is nonzero for the same pair.
    pub static ref INDEXER_CATCHUP_PROGRESS: GaugeVec = register_gauge_vec!(
        "interchain_indexer_catchup_progress",
        "scanned share of the historical block range, 0-100",
        &["bridge_id", "chain_id"],
    )
    .unwrap();

    /// Blocks still inside the unscanned catch-up interval, per bridge and chain.
    pub static ref INDEXER_CATCHUP_BLOCKS_REMAINING: GaugeVec = register_gauge_vec!(
        "interchain_indexer_catchup_blocks_remaining",
        "blocks remaining in the unscanned catchup interval",
        &["bridge_id", "chain_id"],
    )
    .unwrap();

    /// Size of AMB's `pending_message_hash_events` correlation queue, per
    /// bridge.
    ///
    /// This map holds destination-side events that arrived before their source
    /// request, and it is drained only by `handle_source_request`. It has no
    /// TTL, no cap and no offload, so its occupancy is proportional to how far
    /// the *source* chain lags the destination chain — and per-chain
    /// decoupling makes a large, permanent lag a supported configuration
    /// rather than an incident. A monotonically rising series here is the
    /// signal that the queue needs a retention policy; there is nothing today
    /// that bounds it.
    ///
    /// Set from the AMB event handlers on every insert and drain, which is
    /// where the only mutations happen — not from a request handler.
    pub static ref AMB_PENDING_CORRELATION_QUEUE: GaugeVec = register_gauge_vec!(
        "interchain_indexer_amb_pending_correlation_queue",
        "queued AMB destination-side events awaiting their source request",
        &["bridge_id"],
    )
    .unwrap();
}
