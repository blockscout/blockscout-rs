use crate::{
    metrics::Metrics,
    routers::{configure_router, AppRouter},
    settings::Settings,
};
use actix_web::{App, HttpServer};
use futures::future;
use std::sync::Arc;
use tracing_actix_web::TracingLogger;

pub async fn run(settings: Settings) -> std::io::Result<()> {
    let socket_addr = settings.server.addr;
    let metrics_enabled = settings.metrics.enabled;
    let metrics_addr = settings.metrics.addr;
    let metrics_endpoint = settings.metrics.route.clone();

    tracing::info!("smart-contract verifier is starting at {}", socket_addr);
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
    let mut futures = vec![tokio::spawn(server_future)];
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
