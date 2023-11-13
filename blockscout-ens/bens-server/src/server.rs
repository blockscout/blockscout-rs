use crate::{
    services::{domain_extractor::DomainsExtractorService, health::HealthService},
    settings::Settings,
};
use anyhow::Context;
use bens_logic::subgraphs_reader::{blockscout::BlockscoutClient, SubgraphReader};
use bens_proto::blockscout::bens::v1::{
    domains_extractor_actix::route_domains_extractor,
    domains_extractor_server::DomainsExtractorServer, health_actix::route_health,
    health_server::HealthServer,
};
use blockscout_service_launcher::{launcher, launcher::LaunchSettings};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

const SERVICE_NAME: &str = "bens";

#[derive(Clone)]
struct Router {
    domains_extractor: Arc<DomainsExtractorService>,
    health: Arc<HealthService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(DomainsExtractorServer::from_arc(
                self.domains_extractor.clone(),
            ))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config
            .configure(|config| route_domains_extractor(config, self.domains_extractor.clone()));
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &settings.tracing,
        &settings.jaeger,
    )?;

    let health = Arc::new(HealthService::default());

    let database_url = settings.database.connect.url();
    let pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(40)
            .connect(&database_url)
            .await
            .context("database connect")?,
    );
    let blockscout_clients = settings
        .blockscout
        .networks
        .into_iter()
        .map(|(id, network)| {
            (
                id,
                BlockscoutClient::new(
                    network.url,
                    settings.blockscout.max_concurrent_requests,
                    settings.blockscout.timeout,
                ),
            )
        })
        .collect();

    tracing::info!("found blockscout clients from config: {blockscout_clients:?}");

    let subgraph_reader = SubgraphReader::initialize(pool, blockscout_clients)
        .await
        .context("failed to initialize subgraph-reader")?;

    let domains_extractor = Arc::new(DomainsExtractorService::new(subgraph_reader));

    let router = Router {
        domains_extractor,
        health,
    };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };

    launcher::launch(&launch_settings, http_router, grpc_router).await
}
