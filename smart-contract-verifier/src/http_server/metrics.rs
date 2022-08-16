use actix_web::{dev::Server, App, HttpServer};
use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use lazy_static::lazy_static;
use prometheus::{
    register_histogram_with_registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry, Histogram, IntCounter, IntCounterVec, Registry,
};
use std::net::SocketAddr;

use crate::VerificationStatus;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    pub static ref VERIFICATION: IntCounterVec = register_int_counter_vec_with_registry!(
        "verify_contract",
        "number of contract verifications",
        &["language", "endpoint", "status"],
        REGISTRY,
    )
    .unwrap();
    pub static ref DOWNLOAD_CACHE_TOTAL: IntCounter = register_int_counter_with_registry!(
        "download_cache_total",
        "total number of get calls in DownloadCache",
        REGISTRY,
    )
    .unwrap();
    pub static ref DOWNLOAD_CACHE_HITS: IntCounter = register_int_counter_with_registry!(
        "download_cache_hits",
        "number of cache hits in DownloadCache",
        REGISTRY,
    )
    .unwrap();
    pub static ref COMPILER_FETCH_TIME: Histogram = register_histogram_with_registry!(
        "compiler_fetch_time",
        "download time for compilers",
        vec![0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0, 20.0],
        REGISTRY,
    )
    .unwrap();
    pub static ref COMPILE_TIME: Histogram =
        register_histogram_with_registry!("compile_time", "contract compilation time", REGISTRY,)
            .unwrap();
}

pub fn count_verify_contract(status: &VerificationStatus, method: &str) {
    let status = match status {
        VerificationStatus::Ok => "ok",
        VerificationStatus::Failed => "fail",
    };
    VERIFICATION
        .with_label_values(&["solidity", method, status])
        .inc();
}

#[derive(Clone)]
pub struct Metrics {
    metrics_middleware: PrometheusMetrics,
    verification_middleware: PrometheusMetrics,
}

impl Metrics {
    pub fn new(endpoint: String) -> Self {
        let metrics_middleware = PrometheusMetricsBuilder::new("smart_contract_verifier_metrics")
            .registry(REGISTRY.clone())
            .endpoint(&endpoint)
            .build()
            .unwrap();
        // note: verification middleware has no endpoint
        let verification_middleware = PrometheusMetricsBuilder::new("smart_contract_verifier")
            .registry(REGISTRY.clone())
            .build()
            .unwrap();

        Self {
            metrics_middleware,
            verification_middleware,
        }
    }

    pub fn middleware(&self) -> &PrometheusMetrics {
        &self.verification_middleware
    }

    pub fn run_server(&self, addr: SocketAddr) -> Server {
        let metrics_middleware = self.metrics_middleware.clone();
        HttpServer::new(move || App::new().wrap(metrics_middleware.clone()))
            .bind(addr)
            .unwrap()
            .run()
    }
}
