use std::time::Duration;

/// Metrics producer for this exact data source. Does not care about dependencies
/// and such, only tracks this node/source.
pub trait DataSourceMetrics {
    /// Record query time
    fn observe_query_time(time: Duration);
}

// not implementing for () and tuple because it does not make sense
// and `DataSourceMetrics` does not use relations from `DataSource`
