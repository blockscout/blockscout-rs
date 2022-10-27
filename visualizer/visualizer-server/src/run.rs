use crate::{
    health::HealthService,
    metrics::Metrics,
    proto::blockscout::visualizer::v1::{
        health_actix::route_health, health_server::HealthServer,
        solidity_visualizer_server::SolidityVisualizerServer,
    },
    solidity::{route_solidity_visualizer, SolidityVisualizerService},
    tracer::init_logs,
    Settings,
};
use actix_web::{dev::Server, App, HttpServer};
use actix_web_prom::PrometheusMetrics;
use std::{net::SocketAddr, sync::Arc};

pub fn http_server(
    visualizer: Arc<SolidityVisualizerService>,
    health: Arc<HealthService>,
    metrics: PrometheusMetrics,
    addr: SocketAddr,
) -> Server {
    tracing::info!("starting http server on addr {}", addr);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(metrics.clone())
            .configure(|config| route_solidity_visualizer(config, visualizer.clone()))
            .configure(|config| route_health(config, health.clone()))
    })
    .bind(addr)
    .unwrap_or_else(|_| panic!("failed to bind server"));

    server.run()
}

pub async fn grpc_server(
    visualizer: Arc<SolidityVisualizerService>,
    health: Arc<HealthService>,
    addr: SocketAddr,
) -> Result<(), anyhow::Error> {
    tracing::info!("starting grpc server on addr {}", addr);
    let server = tonic::transport::Server::builder()
        .add_service(SolidityVisualizerServer::from_arc(visualizer))
        .add_service(HealthServer::from_arc(health));

    server.serve(addr).await?;
    Ok(())
}

pub async fn sol2uml(settings: Settings) -> Result<(), anyhow::Error> {
    init_logs(settings.jaeger);

    let visualizer = Arc::new(SolidityVisualizerService::default());
    let health = Arc::new(HealthService::default());
    let metrics = Metrics::new(settings.metrics.route);
    let mut futures = vec![];

    if settings.server.http.enabled {
        let http_server = {
            let http_server_future = http_server(
                visualizer.clone(),
                health.clone(),
                metrics.middleware().clone(),
                settings.server.http.addr,
            );
            tokio::spawn(async move { http_server_future.await.map_err(anyhow::Error::msg) })
        };
        futures.push(http_server)
    }

    if settings.server.grpc.enabled {
        let grpc_server = {
            let service = visualizer.clone();
            tokio::spawn(
                async move { grpc_server(service, health, settings.server.grpc.addr).await },
            )
        };
        futures.push(grpc_server)
    }

    if settings.metrics.enabled {
        futures.push(tokio::spawn(async move {
            metrics.run_server(settings.metrics.addr).await?;
            Ok(())
        }))
    }

    let (res, _, others) = futures::future::select_all(futures).await;
    for future in others.into_iter() {
        future.abort()
    }
    res?
}
