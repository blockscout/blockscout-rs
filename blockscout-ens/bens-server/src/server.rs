use crate::{
    jobs,
    services::{domain_extractor::DomainsExtractorService, health::HealthService},
    settings::Settings,
};
use anyhow::Context;
use bens_logic::{
    blockscout::BlockscoutClient,
    protocols::{Network, ProtocolInfo},
    subgraph::SubgraphReader,
};
use bens_proto::blockscout::bens::v1::{
    domains_extractor_actix::route_domains_extractor,
    domains_extractor_server::DomainsExtractorServer, health_actix::route_health,
    health_server::HealthServer,
};
use blockscout_service_launcher::{launcher, launcher::LaunchSettings};
use sqlx::postgres::PgPoolOptions;
use std::{collections::HashMap, sync::Arc};
use tokio_cron_scheduler::JobScheduler;

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
    if settings.database.run_migrations {
        tracing::info!("running migrations");
        bens_logic::migrations::run(&pool).await?;
    }
    let networks = settings
        .subgraphs_reader
        .networks
        .into_iter()
        .map(|(id, network)| {
            let blockscout_client = Arc::new(BlockscoutClient::new(
                network.blockscout.url,
                network.blockscout.max_concurrent_requests,
                network.blockscout.timeout,
            ));
            (
                id,
                Network {
                    blockscout_client,
                    use_protocols: network.use_protocols,
                    rpc_url: network.rpc_url,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    tracing::info!(
        "networks from config: {}",
        serde_json::json!(networks
            .iter()
            .map(|(id, n)| (id, n.use_protocols.iter().collect::<Vec<_>>()))
            .collect::<HashMap<_, _>>())
    );
    let protocols = settings
        .subgraphs_reader
        .protocols
        .into_iter()
        .filter(|(_, p)| !p.disabled)
        .map(|(name, p)| {
            (
                name.clone(),
                ProtocolInfo {
                    slug: name,
                    network_id: p.network_id,
                    tld_list: p.tld_list,
                    subgraph_name: p.subgraph_name,
                    address_resolve_technique: p.address_resolve_technique,
                    meta: p.meta.0,
                    protocol_specific: p.protocol_specific.0,
                },
            )
        })
        .collect::<HashMap<_, _>>();

    tracing::info!(
        "protocols from config: {:?}",
        protocols.keys().collect::<Vec<_>>()
    );

    let subgraph_reader = SubgraphReader::initialize(pool, networks, protocols)
        .await
        .context("failed to initialize subgraph-reader")?;
    let subgraph_reader = Arc::new(subgraph_reader);
    let domains_extractor = Arc::new(DomainsExtractorService::new(subgraph_reader.clone()));

    let scheduler = JobScheduler::new().await?;
    scheduler
        .add(jobs::refresh_cache_job(
            &settings.subgraphs_reader.refresh_cache_schedule,
            subgraph_reader.clone(),
        )?)
        .await?;
    tracing::info!("starting job scheduler");
    scheduler.start().await?;

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
        graceful_shutdown: Default::default(),
    };

    tracing::info!("launching web service");
    launcher::launch(launch_settings, http_router, grpc_router).await
}
