use crate::verification_response::VerificationStatus;
use actix_web::{dev::Server, App, HttpServer};
use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, IntCounterVec};
use std::net::SocketAddr;

lazy_static! {
    pub static ref VERIFICATION: IntCounterVec = register_int_counter_vec!(
        "smart_contract_verifier_verify_contract",
        "number of contract verifications",
        &["language", "endpoint", "status"],
    )
    .unwrap();
}

pub fn count_verify_contract(language: &str, status: &VerificationStatus, method: &str) {
    let status = match status {
        VerificationStatus::Ok => "ok",
        VerificationStatus::Failed => "fail",
    };
    VERIFICATION
        .with_label_values(&[language, method, status])
        .inc();
}

#[derive(Clone)]
pub struct Metrics {
    metrics_middleware: PrometheusMetrics,
    verification_middleware: PrometheusMetrics,
}

impl Metrics {
    pub fn new(endpoint: String) -> Self {
        let regustry = prometheus::default_registry();
        let metrics_middleware = PrometheusMetricsBuilder::new("smart_contract_verifier_metrics")
            .registry(regustry.clone())
            .endpoint(&endpoint)
            .build()
            .unwrap();
        // note: verification middleware has no endpoint
        let verification_middleware = PrometheusMetricsBuilder::new("smart_contract_verifier")
            .registry(regustry.clone())
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
