use lazy_static::lazy_static;
use prometheus::{
    GaugeVec, HistogramVec, IntCounterVec, register_gauge_vec, register_histogram_vec,
    register_int_counter_vec,
};

// Metrics for the message buffer. Keep labels low-cardinality: bridge_id and chain_id.
lazy_static! {
    /// Number of entries currently in the hot tier (DashMap) per bridge.
    pub static ref BUFFER_HOT_ENTRIES: GaugeVec = register_gauge_vec!(
        "interchain_indexer_buffer_hot_entries",
        "current number of hot-tier entries held in memory",
        &["bridge_id"],
    )
    .unwrap();

    /// Number of pending messages stored in cold tier per bridge.
    pub static ref BUFFER_PENDING_ENTRIES: GaugeVec = register_gauge_vec!(
        "interchain_indexer_buffer_pending_entries",
        "current number of pending messages stored in cold tier",
        &["bridge_id"],
    )
    .unwrap();

    /// Number of entries flushed in a maintenance run (bucketed) per bridge.
    pub static ref BUFFER_FLUSH_FINAL_ENTRIES: HistogramVec = register_histogram_vec!(
        "interchain_indexer_buffer_flush_final_entries",
        "entries flushed in a single maintenance run",
        &["bridge_id"],
        vec![0.0, 1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]
    )
    .unwrap();

    /// Catchup cursor (blocks) per bridge/chain.
    pub static ref BUFFER_CATCHUP_CURSOR: GaugeVec = register_gauge_vec!(
        "interchain_indexer_buffer_catchup_cursor",
        "catchup cursor height per chain",
        &["bridge_id", "chain_id"],
    )
    .unwrap();

    /// Realtime cursor (blocks) per bridge/chain.
    pub static ref BUFFER_REALTIME_CURSOR: GaugeVec = register_gauge_vec!(
        "interchain_indexer_buffer_realtime_cursor",
        "realtime cursor height per chain",
        &["bridge_id", "chain_id"],
    )
    .unwrap();

    /// Restore attempts from cold tier with result label {hit, miss, error}.
    pub static ref BUFFER_RESTORE_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_buffer_restore_total",
        "restore attempts from cold tier",
        &["bridge_id", "result"],
    )
    .unwrap();
}
