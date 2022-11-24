use crate::{
    metrics::Metrics,
    services::{
        HealthService, SolidityVerifierService, SourcifyVerifierService, VyperVerifierService,
    },
    settings::Settings,
    tracing::init_logs,
};
use actix_web::{dev::Server, App, HttpServer};
use actix_web_prom::PrometheusMetrics;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    health_actix::route_health, health_server::HealthServer,
    solidity_verifier_actix::route_solidity_verifier,
    solidity_verifier_server::SolidityVerifierServer,
    sourcify_verifier_actix::route_sourcify_verifier,
    sourcify_verifier_server::SourcifyVerifierServer, vyper_verifier_actix::route_vyper_verifier,
    vyper_verifier_server::VyperVerifierServer,
};
use std::{net::SocketAddr, sync::Arc};

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    init_logs(settings.jaeger);

    let solidity_verifier = Arc::new(SolidityVerifierService::default());
    let vyper_verifier = Arc::new(VyperVerifierService::default());
    let sourcify_verifier = Arc::new(SourcifyVerifierService::default());
    let health = Arc::new(HealthService::default());
    let metrics = Metrics::new(settings.metrics.route);
    let mut futures = vec![];

    if settings.server.http.enabled {
        let http_server = {
            let http_server_future = http_server(
                solidity_verifier.clone(),
                vyper_verifier.clone(),
                sourcify_verifier.clone(),
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
            tokio::spawn(async move {
                grpc_server(
                    solidity_verifier,
                    vyper_verifier,
                    sourcify_verifier,
                    health,
                    settings.server.grpc.addr,
                )
                .await
            })
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

pub fn http_server(
    solidity_verifier: Arc<SolidityVerifierService>,
    vyper_verifier: Arc<VyperVerifierService>,
    sourcify_verifier: Arc<SourcifyVerifierService>,
    health: Arc<HealthService>,
    metrics: PrometheusMetrics,
    addr: SocketAddr,
) -> Server {
    tracing::info!("starting http server on addr {}", addr);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(metrics.clone())
            .configure(|config| route_solidity_verifier(config, solidity_verifier.clone()))
            .configure(|config| route_vyper_verifier(config, vyper_verifier.clone()))
            .configure(|config| route_sourcify_verifier(config, sourcify_verifier.clone()))
            .configure(|config| route_health(config, health.clone()))
    })
    .bind(addr)
    .unwrap_or_else(|_| panic!("failed to bind server"));

    server.run()
}

pub async fn grpc_server(
    solidity_verifier: Arc<SolidityVerifierService>,
    vyper_verifier: Arc<VyperVerifierService>,
    sourcify_verifier: Arc<SourcifyVerifierService>,
    health: Arc<HealthService>,
    addr: SocketAddr,
) -> Result<(), anyhow::Error> {
    tracing::info!("starting grpc server on addr {}", addr);
    let server = tonic::transport::Server::builder()
        .add_service(SolidityVerifierServer::from_arc(solidity_verifier))
        .add_service(VyperVerifierServer::from_arc(vyper_verifier))
        .add_service(SourcifyVerifierServer::from_arc(sourcify_verifier))
        .add_service(HealthServer::from_arc(health));

    server.serve(addr).await?;
    Ok(())
}
