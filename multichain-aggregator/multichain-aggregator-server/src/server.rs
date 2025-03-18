use crate::{
    proto::{
        health_actix::route_health, health_server::HealthServer,
        multichain_aggregator_service_actix::route_multichain_aggregator_service,
        multichain_aggregator_service_server::MultichainAggregatorServiceServer,
    },
    services::{HealthService, MultichainAggregator, ReadWriteRepo},
    settings::Settings,
};
use blockscout_chains::BlockscoutChainsClient;
use blockscout_service_launcher::{
    database,
    launcher::{self, LaunchSettings},
};
use migration::Migrator;
use multichain_aggregator_logic::{
    clients::{dapp, token_info},
    repository,
};
use std::sync::Arc;

const SERVICE_NAME: &str = "multichain_aggregator";

#[derive(Clone)]
struct Router {
    multichain_aggregator: Arc<MultichainAggregator>,
    health: Arc<HealthService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(MultichainAggregatorServiceServer::from_arc(
                self.multichain_aggregator.clone(),
            ))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config.configure(|config| {
            route_multichain_aggregator_service(config, self.multichain_aggregator.clone())
        });
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &settings.tracing,
        &settings.jaeger,
    )?;

    let health = Arc::new(HealthService::default());

    let db = database::initialize_postgres::<Migrator>(&settings.database).await?;
    let replica_db = if let Some(db) = settings.replica_database {
        Some(database::initialize_postgres::<Migrator>(&db).await?)
    } else {
        None
    };

    let repo = ReadWriteRepo::new(db, replica_db);

    // Initialize/update Blockscout chains
    let blockscout_chains = BlockscoutChainsClient::builder()
        .with_max_retries(0)
        .build()
        .fetch_all()
        .await?
        .into_iter()
        .filter_map(|(id, chain)| {
            let id = id.parse::<i64>().ok()?;
            Some((id, chain).into())
        })
        .collect::<Vec<_>>();
    repository::chains::upsert_many(repo.write_db(), blockscout_chains).await?;

    let dapp_client = dapp::new_client(settings.service.dapp_client.url)?;
    let token_info_client = token_info::new_client(settings.service.token_info_client.url)?;

    let multichain_aggregator = Arc::new(MultichainAggregator::new(
        repo,
        dapp_client,
        token_info_client,
        settings.service.api,
        settings.service.quick_search_chains,
    ));

    let router = Router {
        health,
        multichain_aggregator,
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
