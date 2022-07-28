pub mod handlers;
pub mod metrics;
mod routers;

pub use self::routers::{configure_router, AppRouter, Router};

use crate::config::Config;
use actix_web::{App, HttpServer};

use futures::future;
use metrics::Metrics;
use std::sync::Arc;

pub async fn run(config: Config) -> std::io::Result<()> {
    let socket_addr = config.server.addr;
    let metrics_addr = config.metrics.addr;
    let metrics_endpoint = config.metrics.endpoint.clone();

    log::info!("Verification server is starting at {}", socket_addr);
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
                .configure(configure_router(&*app_router))
        })
        .bind(socket_addr)?
        .run()
    };
    let server_future = tokio::spawn(async move { server_future.await });
    let metrics_future = tokio::spawn(async move { metrics.run_server(metrics_addr).await });

    let (server_future, metrics_future) = future::try_join(server_future, metrics_future).await?;

    server_future?;
    metrics_future?;
    Ok(())
}
