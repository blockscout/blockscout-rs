pub mod handlers;
pub mod metrics;
mod routers;

pub use self::routers::{configure_router, AppRouter, Router};
use tracing_actix_web::TracingLogger;

use crate::Settings;
use actix_web::{App, HttpServer};

use futures::future;
use metrics::Metrics;
use std::sync::Arc;

pub async fn run(settings: Settings) -> std::io::Result<()> {
    let socket_addr = settings.server.addr;
    let metrics_enabled = settings.metrics.enabled;
    let metrics_addr = settings.metrics.addr;
    let metrics_endpoint = settings.metrics.route.clone();

    tracing::info!("Smart-contract verifier is starting at {}", socket_addr);
    let app_router = Arc::new(
        AppRouter::new(settings)
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
