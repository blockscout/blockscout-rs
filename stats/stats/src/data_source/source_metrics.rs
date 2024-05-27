use std::time::Duration;

/// Metrics producer for this exact data source. Does not care about dependencies
/// and such, only tracks this node/source.
///
/// Implemented automatically for all `UpdateableChartDataSourceWrapper`s.
/// Do not bother with it unless implementing `DataSource` trait manually.
pub trait DataSourceMetrics {
    /// Record query time
    fn observe_query_time(time: Duration);
}

// implementing for () and tuple does not make sense
// + `DataSourceMetrics` does not use relations from `DataSource`
