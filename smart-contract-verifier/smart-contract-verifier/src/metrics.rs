// SPDX-License-Identifier: LicenseRef-Blockscout

use lazy_static::lazy_static;
use prometheus::{
    register_gauge, register_histogram, register_histogram_vec, register_int_counter,
    register_int_counter_vec, register_int_gauge_vec, Gauge, Histogram, HistogramVec, IntCounter,
    IntCounterVec, IntGaugeVec,
};

const COMPILER_RUNNER_OPERATION_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0,
];

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
        "compiler invocation time in seconds, including compiler version probes",
    )
    .unwrap();
    pub static ref COMPILATIONS_IN_FLIGHT: Gauge = register_gauge!(
        "smart_contract_verifier_compiles_in_flight",
        "number of compiler invocations currently running",
    )
    .unwrap();
    pub static ref COMPILATION_QUEUE_TIME: Histogram = register_histogram!(
        "smart_contract_verifier_compilation_queue_time_seconds",
        "time waiting for the shared compiler invocation limit in seconds",
    )
    .unwrap();
    pub static ref COMPILATIONS_IN_QUEUE: Gauge = register_gauge!(
        "smart_contract_verifier_compiles_in_queue",
        "number of compiler invocations waiting for the shared limit",
    )
    .unwrap();
    pub static ref COMPILER_RUNNER_JOBS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "smart_contract_verifier_compiler_runner_jobs_total",
        "compiler runner jobs classified by terminal outcome",
        &["executor", "outcome"],
    )
    .unwrap();
    pub static ref COMPILER_RUNNER_OPERATION_DURATION: HistogramVec = register_histogram_vec!(
        "smart_contract_verifier_compiler_runner_operation_duration_seconds",
        "time spent in compiler runner operations, including failed and canceled operations",
        &["executor", "operation", "family"],
        COMPILER_RUNNER_OPERATION_BUCKETS.to_vec(),
    )
    .unwrap();
    pub static ref COMPILER_RUNNER_TRANSFER_BYTES: IntCounterVec = register_int_counter_vec!(
        "smart_contract_verifier_compiler_runner_transfer_bytes_total",
        "bytes successfully transferred across compiler executor boundaries",
        &["executor", "direction", "family"],
    )
    .unwrap();
    pub static ref COMPILER_RUNNER_ACTIVE_CONTAINERS: IntGaugeVec = register_int_gauge_vec!(
        "smart_contract_verifier_compiler_runner_active_containers",
        "Docker container lifecycles currently managed by this verifier process",
        &["family"],
    )
    .unwrap();
    pub static ref COMPILER_RUNNER_MAX_CONCURRENT_JOBS: IntGaugeVec = register_int_gauge_vec!(
        "smart_contract_verifier_compiler_runner_max_concurrent_jobs",
        "configured compiler runner concurrency; sum across replicas gives runner slot capacity",
        &["executor"],
    )
    .unwrap();
    pub static ref COMPILER_RUNNER_JOBS_CURRENT: IntGaugeVec = register_int_gauge_vec!(
        "smart_contract_verifier_compiler_runner_jobs_current",
        "compiler runner jobs currently queued or holding a runner concurrency slot",
        &["executor", "state"],
    )
    .unwrap();
    pub static ref COMPILER_RUNNER_ORPHAN_CLEANUP_SWEEPS_TOTAL: IntCounterVec =
        register_int_counter_vec!(
            "smart_contract_verifier_compiler_runner_orphan_cleanup_sweeps_total",
            "compiler container orphan-cleanup sweeps by trigger and outcome",
            &["trigger", "outcome"],
        )
        .unwrap();
    pub static ref COMPILER_RUNNER_ORPHAN_CLEANUP_CONTAINERS_TOTAL: IntCounterVec =
        register_int_counter_vec!(
            "smart_contract_verifier_compiler_runner_orphan_cleanup_containers_total",
            "compiler containers observed by orphan cleanup, classified by family and outcome",
            &["family", "outcome"],
        )
        .unwrap();
}

pub struct GaugeGuard(&'static Gauge);

impl Drop for GaugeGuard {
    fn drop(&mut self) {
        self.0.dec();
    }
}

pub trait GuardedGauge {
    fn guarded_inc(&'static self) -> GaugeGuard;
}

impl GuardedGauge for Gauge {
    fn guarded_inc(&'static self) -> GaugeGuard {
        self.inc();
        GaugeGuard(self)
    }
}
