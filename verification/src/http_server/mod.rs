pub mod handlers;
pub mod metrics;
mod routers;

pub use self::routers::{configure_router, AppRouter, Router};
use tracing_actix_web::TracingLogger;

use crate::config::Config;
use actix_web::{App, HttpServer};

use futures::future;
use metrics::Metrics;
use std::sync::Arc;

pub async fn run(config: Config) -> std::io::Result<()> {
    let socket_addr = config.server.addr;
    let metrics_enabled = config.metrics.enabled;
    let metrics_addr = config.metrics.addr;
    let metrics_endpoint = config.metrics.endpoint.clone();

    tracing::info!("Verification server is starting at {}", socket_addr);
    let app_router = Arc::new(
        AppRouter::new(config)
            .await
            .expect("couldn't initialize the app"),
    );
    let metrics = Metrics::new(metrics_endpoint);
    let server_future = {
        let middleware = metrics.middleware().clone();
        HttpServer::new(move || {
            App::new()
                .wrap(middleware.clone())
                .wrap(TracingLogger::default())
                .configure(configure_router(&*app_router))
        })
        .bind(socket_addr)?
        .run()
    };
    let mut futures = vec![tokio::spawn(async move { server_future.await })];
    if metrics_enabled {
        futures.push(tokio::spawn(async move {
            metrics.run_server(metrics_addr).await
        }))
    }
    let (res, _, others) = future::select_all(futures).await;
    for future in others.into_iter() {
        future.abort()
    }
    res?
}
