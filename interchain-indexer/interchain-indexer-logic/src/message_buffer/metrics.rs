use lazy_static::lazy_static;
use prometheus::{
    GaugeVec, Histogram, HistogramVec, IntCounter, IntCounterVec, register_gauge_vec,
    register_histogram, register_histogram_vec, register_int_counter, register_int_counter_vec,
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

    /// Time spent executing a maintenance cycle.
    pub static ref BUFFER_MAINTENANCE_DURATION: Histogram = register_histogram!(
        "interchain_indexer_buffer_maintenance_duration_seconds",
        "maintenance cycle duration",
        vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
    )
    .unwrap();

    /// Counts of entries encountered in maintenance, bucketed by state.
    pub static ref BUFFER_MAINTENANCE_ENTRIES: GaugeVec = register_gauge_vec!(
        "interchain_indexer_buffer_maintenance_entries",
        "entries encountered during maintenance grouped by state",
        &["bridge_id", "state"],
    )
    .unwrap();

    /// Maintenance failures.
    pub static ref BUFFER_MAINTENANCE_ERRORS_TOTAL: IntCounter = register_int_counter!(
        "interchain_indexer_buffer_maintenance_errors_total",
        "maintenance loop errors"
    )
    .unwrap();

    /// Evictions skipped due to concurrent modification, per bridge.
    pub static ref BUFFER_EVICTION_SKIPPED_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_buffer_eviction_skipped_total",
        "evictions skipped due to concurrent modification",
        &["bridge_id"],
    )
    .unwrap();

    /// Messages finalized into crosschain_messages per bridge.
    pub static ref BUFFER_MESSAGES_FINALIZED_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_messages_finalized_total",
        "messages finalized into crosschain_messages",
        &["bridge_id"],
    )
    .unwrap();

    /// Transfers finalized into crosschain_transfers per bridge.
    pub static ref BUFFER_TRANSFERS_FINALIZED_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_transfers_finalized_total",
        "transfers finalized into crosschain_transfers",
        &["bridge_id"],
    )
    .unwrap();

    /// Entries evicted from hot tier in a maintenance run, per bridge and reason.
    pub static ref BUFFER_EVICTED_ENTRIES: HistogramVec = register_histogram_vec!(
        "interchain_indexer_buffer_evicted_entries",
        "entries evicted from hot tier in a single maintenance run",
        &["bridge_id", "reason"],
        vec![0.0, 1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]
    )
    .unwrap();

    /// Cursor block height per bridge, chain, and direction.
    pub static ref BUFFER_CURSOR: GaugeVec = register_gauge_vec!(
        "interchain_indexer_buffer_cursor",
        "cursor block height per bridge and chain",
        &["bridge_id", "chain_id", "direction"],
    )
    .unwrap();

    /// Restore attempts from cold tier with result label {hit, miss}.
    pub static ref BUFFER_RESTORE_TOTAL: IntCounterVec = register_int_counter_vec!(
        "interchain_indexer_buffer_restore_total",
        "restore attempts from cold tier",
        &["bridge_id", "result"],
    )
    .unwrap();
}
