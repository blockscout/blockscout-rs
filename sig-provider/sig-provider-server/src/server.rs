use crate::{
    health::HealthService, metrics::Metrics, settings::SourcesSettings, tracing::init_logs,
    Service, Settings,
};
use actix_web::{web::ServiceConfig, App, HttpServer};
use actix_web_prom::PrometheusMetrics;
use sig_provider::{fourbyte, sigeth, SourceAggregator};
use sig_provider_proto::blockscout::sig_provider::v1::{
    abi_service_actix::route_abi_service,
    abi_service_server::{AbiService, AbiServiceServer},
    health_actix::route_health,
    signature_service_actix::route_signature_service,
    signature_service_server::{SignatureService, SignatureServiceServer},
};
use std::{net::SocketAddr, sync::Arc};

pub fn http_configure<S: SignatureService, A: AbiService>(
    config: &mut ServiceConfig,
    signature: Arc<S>,
    abi: Arc<A>,
) {
    route_signature_service(config, signature);
    route_abi_service(config, abi);
}

pub fn http_server<S: SignatureService, A: AbiService>(
    signature: Arc<S>,
    abi: Arc<A>,
    health: Arc<HealthService>,
    metrics: PrometheusMetrics,
    addr: SocketAddr,
) -> actix_web::dev::Server {
    tracing::info!("starting http server on addr {}", addr);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(metrics.clone())
            .configure(|config| http_configure(config, signature.clone(), abi.clone()))
            .configure(|config| route_health(config, health.clone()))
    })
    .bind(addr)
    .unwrap_or_else(|_| panic!("failed to bind server"));

    server.run()
}

pub fn grpc_server<S: SignatureService, A: AbiService>(
    signature: Arc<S>,
    abi: Arc<A>,
    addr: SocketAddr,
) -> impl futures::Future<Output = Result<(), tonic::transport::Error>> {
    tracing::info!("starting grpc server on addr {}", addr);
    let server = tonic::transport::Server::builder()
        .add_service(SignatureServiceServer::from_arc(signature))
        .add_service(AbiServiceServer::from_arc(abi));

    server.serve(addr)
}

pub fn new_service(sources: SourcesSettings) -> Arc<Service> {
    let aggregator = Arc::new(SourceAggregator::new(vec![
        Arc::new(sigeth::Source::new(sources.sigeth)),
        Arc::new(fourbyte::Source::new(sources.fourbyte)),
    ]));
    Arc::new(Service::new(aggregator))
}

pub async fn sig_provider(settings: Settings) -> Result<(), anyhow::Error> {
    init_logs(settings.jaeger);

    let service = new_service(settings.sources);
    let health = Arc::new(HealthService::default());
    let metrics = Metrics::new(settings.metrics.route);

    let mut futures = vec![];

    if settings.server.http.enabled {
        let http_server = {
            let http_server_future = http_server(
                service.clone(),
                service.clone(),
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
            let grpc_server_future =
                grpc_server(service.clone(), service.clone(), settings.server.grpc.addr);
            tokio::spawn(async move { grpc_server_future.await.map_err(anyhow::Error::msg) })
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
