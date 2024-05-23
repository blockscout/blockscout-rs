use lazy_static::lazy_static;
use prometheus::{register_histogram_vec, register_int_counter_vec, HistogramVec, IntCounterVec};

lazy_static! {
    pub static ref UPDATE_ERRORS: IntCounterVec = register_int_counter_vec!(
        "stats_update_errors_total",
        "total update errors",
        &["chart_id"],
    )
    .unwrap();
    pub static ref CHART_UPDATE_TIME: HistogramVec = register_histogram_vec!(
        "stats_chart_update_time_seconds",
        "single chart update time",
        &["chart_id"],
        vec![1.0, 2.0, 4.0, 8.0, 16.0, 30.0, 60.0, 120.0, 240.0, 480.0, 960.0, 1920.0, 3840.0],
    )
    .unwrap();
    pub static ref CHART_FETCH_NEW_DATA_TIME: HistogramVec = register_histogram_vec!(
        "stats_fetch_new_data_time_seconds",
        "single chart time for fetching data from blockscout",
        &["chart_id"],
        vec![1.0, 2.0, 4.0, 8.0, 16.0, 30.0, 60.0, 120.0, 240.0, 480.0, 960.0, 1920.0, 3840.0],
    )
    .unwrap();
}
