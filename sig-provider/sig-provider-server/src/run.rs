use crate::{health::HealthService, settings::SourcesSettings, Service, Settings};
use actix_web::web::ServiceConfig;
use sig_provider::{fourbyte, sigeth, SourceAggregator};
use sig_provider_proto::blockscout::sig_provider::v1::{
    abi_service_actix::route_abi_service,
    abi_service_server::{AbiService, AbiServiceServer},
    health_actix::route_health,
    health_server::HealthServer,
    signature_service_actix::route_signature_service,
    signature_service_server::{SignatureService, SignatureServiceServer},
};
use std::sync::Arc;

pub fn http_configure<S: SignatureService, A: AbiService>(
    config: &mut ServiceConfig,
    signature: Arc<S>,
    abi: Arc<A>,
) {
    route_signature_service(config, signature);
    route_abi_service(config, abi);
}

#[derive(Clone)]
struct HttpRouter<S: SignatureService, A: AbiService> {
    signature: Arc<S>,
    abi: Arc<A>,
    health: Arc<HealthService>,
}

impl<S: SignatureService, A: AbiService> startuper::HttpRouter for HttpRouter<S, A> {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config
            .configure(|config| route_health(config, self.health.clone()))
            .configure(|config| http_configure(config, self.signature.clone(), self.abi.clone()));
    }
}

fn grpc_router<S: SignatureService, A: AbiService>(
    signature: Arc<S>,
    abi: Arc<A>,
    health: Arc<HealthService>,
) -> tonic::transport::server::Router {
    tonic::transport::Server::builder()
        .add_service(HealthServer::from_arc(health))
        .add_service(SignatureServiceServer::from_arc(signature))
        .add_service(AbiServiceServer::from_arc(abi))
}

pub fn new_service(sources: SourcesSettings) -> Arc<Service> {
    let aggregator = Arc::new(SourceAggregator::new(vec![
        Arc::new(sigeth::Source::new(sources.sigeth)),
        Arc::new(fourbyte::Source::new(sources.fourbyte)),
    ]));
    Arc::new(Service::new(aggregator))
}

pub async fn sig_provider(settings: Settings) -> Result<(), anyhow::Error> {
    let service = new_service(settings.sources);
    let health = Arc::new(HealthService::default());

    let grpc_router = grpc_router(service.clone(), service.clone(), health.clone());
    let http_router = HttpRouter {
        signature: service.clone(),
        abi: service.clone(),
        health,
    };
    let startup_settings = startuper::StartupSettings {
        service_name: "sig_provider".to_owned(),
        server: settings.server,
        metrics: settings.metrics,
        jaeger: settings.jaeger,
    };

    startuper::start_it_up(&startup_settings, http_router, grpc_router).await
}
