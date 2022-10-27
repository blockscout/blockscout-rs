use actix_web::{dev::Server, App, HttpServer};
use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use std::net::SocketAddr;

#[derive(Clone)]
pub struct Metrics {
    metrics_middleware: PrometheusMetrics,
    visualizer_middleware: PrometheusMetrics,
}

impl Metrics {
    pub fn new(endpoint: String) -> Self {
        let regustry = prometheus::default_registry();
        let metrics_middleware = PrometheusMetricsBuilder::new("visualizer_metrics")
            .registry(regustry.clone())
            .endpoint(&endpoint)
            .build()
            .unwrap();
        let visualizer_middleware = PrometheusMetricsBuilder::new("visualizer")
            .registry(regustry.clone())
            .build()
            .unwrap();

        Self {
            metrics_middleware,
            visualizer_middleware,
        }
    }

    pub fn middleware(&self) -> &PrometheusMetrics {
        &self.visualizer_middleware
    }

    pub fn run_server(&self, addr: SocketAddr) -> Server {
        tracing::info!(addr = ?addr, "starting metris server");
        let metrics_middleware = self.metrics_middleware.clone();
        HttpServer::new(move || App::new().wrap(metrics_middleware.clone()))
            .bind(addr)
            .unwrap()
            .run()
    }
}
