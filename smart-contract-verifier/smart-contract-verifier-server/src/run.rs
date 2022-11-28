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
use tokio::sync::Semaphore;

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    init_logs(settings.jaeger);

    let compilers_lock = Arc::new(Semaphore::new(settings.compilers.max_threads.get()));

    let solidity_verifier = match settings.solidity.enabled {
        true => Some(Arc::new(
            SolidityVerifierService::new(
                settings.solidity,
                compilers_lock.clone(),
                settings.extensions.solidity,
            )
            .await?,
        )),
        false => None,
    };
    let vyper_verifier = settings
        .vyper
        .enabled
        .then(|| Arc::new(VyperVerifierService::default()));
    let sourcify_verifier = settings
        .sourcify
        .enabled
        .then(|| Arc::new(SourcifyVerifierService::default()));
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
    solidity_verifier: Option<Arc<SolidityVerifierService>>,
    vyper_verifier: Option<Arc<VyperVerifierService>>,
    sourcify_verifier: Option<Arc<SourcifyVerifierService>>,
    health: Arc<HealthService>,
    metrics: PrometheusMetrics,
    addr: SocketAddr,
) -> Server {
    tracing::info!("starting http server on addr {}", addr);
    let server = HttpServer::new(move || {
        let app = App::new()
            .wrap(metrics.clone())
            .configure(|config| route_health(config, health.clone()));
        let app = if let Some(solidity_verifier) = &solidity_verifier {
            app.configure(|config| route_solidity_verifier(config, solidity_verifier.clone()))
        } else {
            app
        };
        let app = if let Some(vyper_verifier) = &vyper_verifier {
            app.configure(|config| route_vyper_verifier(config, vyper_verifier.clone()))
        } else {
            app
        };
        if let Some(sourcify_verifier) = &sourcify_verifier {
            app.configure(|config| route_sourcify_verifier(config, sourcify_verifier.clone()))
        } else {
            app
        }
    })
    .bind(addr)
    .unwrap_or_else(|_| panic!("failed to bind server"));

    server.run()
}

pub async fn grpc_server(
    solidity_verifier: Option<Arc<SolidityVerifierService>>,
    vyper_verifier: Option<Arc<VyperVerifierService>>,
    sourcify_verifier: Option<Arc<SourcifyVerifierService>>,
    health: Arc<HealthService>,
    addr: SocketAddr,
) -> Result<(), anyhow::Error> {
    tracing::info!("starting grpc server on addr {}", addr);
    let server = {
        let server =
            tonic::transport::Server::builder().add_service(HealthServer::from_arc(health));
        let server = if let Some(solidity_verifier) = solidity_verifier {
            server.add_service(SolidityVerifierServer::from_arc(solidity_verifier))
        } else {
            server
        };
        let server = if let Some(vyper_verifier) = vyper_verifier {
            server.add_service(VyperVerifierServer::from_arc(vyper_verifier))
        } else {
            server
        };
        if let Some(sourcify_verifier) = sourcify_verifier {
            server.add_service(SourcifyVerifierServer::from_arc(sourcify_verifier))
        } else {
            server
        }
    };

    server.serve(addr).await?;
    Ok(())
}
