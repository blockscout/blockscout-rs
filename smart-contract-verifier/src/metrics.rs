use lazy_static::lazy_static;
use prometheus::{register_histogram, register_int_counter, Histogram, IntCounter};

lazy_static! {
    pub static ref DOWNLOAD_CACHE_TOTAL: IntCounter = register_int_counter!(
        "smart_contract_verifier_download_cache_total",
        "total number of get calls in DownloadCache",
    )
    .unwrap();
    pub static ref DOWNLOAD_CACHE_HITS: IntCounter = register_int_counter!(
        "smart_contract_verifier_download_cache_hits",
        "number of cache hits in DownloadCache",
    )
    .unwrap();
    pub static ref COMPILER_FETCH_TIME: Histogram = register_histogram!(
        "smart_contract_verifier_compiler_fetch_time_seconds",
        "download time for compilers in seconds",
        vec![0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0, 20.0],
    )
    .unwrap();
    pub static ref COMPILE_TIME: Histogram = register_histogram!(
        "smart_contract_verifier_compile_time_seconds",
        "contract compilation time in seconds",
    )
    .unwrap();
}
