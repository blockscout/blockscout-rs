use lazy_static::lazy_static;
use prometheus::{register_histogram_vec, HistogramVec};

lazy_static! {
    pub static ref ALL_MATCHES_COUNT: HistogramVec = register_histogram_vec!(
        "eth_bytecode_db_all_matches_count",
        "number of fully and partially matched contracts",
        &["bytecode_type"],
        [0, 1, 5, 10, 25, 50, 75, 100, 250, 500, 750, 1000, 2500, 5000, 7500, 10000].into_iter().map(|v| v as f64).collect()
    ).unwrap();

    pub static ref MATCHES_SEARCH_TIME: HistogramVec = register_histogram_vec!(
        "eth_bytecode_db_matches_search_time_seconds",
        "all existing matches search time in seconds",
        &["bytecode_type"],
        vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
    ).unwrap();

    pub static ref FULL_MATCHES_COUNT: HistogramVec = register_histogram_vec!(
        "eth_bytecode_db_full_matches_count",
        "number of fully matched contracts",
        &["bytecode_type"],
        [0, 1, 2, 3, 4, 5].into_iter().map(|v| v as f64).collect()
    ).unwrap();

    pub static ref FULL_MATCHES_CHECK_TIME: HistogramVec = register_histogram_vec!(
        "eth_bytecode_db_full_matches_check_time_seconds",
        "the time required for the final matches list to be checked on full matched contracts in seconds",
        &["bytecode_type"],
        vec![0.01, 0.025, 0.05, 0.075, 0.1, 0.2, 0.3, 0.4, 0.5, 0.75, 1.0]
    ).unwrap();

    pub static ref BYTECODE_CANDIDATES_SEARCH_TIME: HistogramVec = register_histogram_vec!(
        "eth_bytecode_db_bytecode_candidates_search_time_seconds",
        "bytecode candidates search time in seconds",
        &["bytecode_type"],
        vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
    ).unwrap();

    pub static ref BYTECODE_CANDIDATES_COUNT: HistogramVec = register_histogram_vec!(
        "eth_bytecode_db_bytecode_candidates_count",
        "number of bytecode candidates",
        &["bytecode_type"],
        [0, 1, 5, 10, 25, 50, 75, 100, 250, 500, 750, 1000, 2500, 5000, 7500, 10000].into_iter().map(|v| v as f64).collect()
    ).unwrap();

    pub static ref MATCHES_BY_CANDIDATES_GET_TIME: HistogramVec = register_histogram_vec!(
        "eth_bytecode_db_matches_by_candidates_get_time_seconds",
        "obataining matches from candidates list time in seconds",
        &["bytecode_type"],
        vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
    ).unwrap();
}
