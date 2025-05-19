use crate::{
    proto::{
        health_actix::route_health, health_server::HealthServer,
        multichain_aggregator_service_actix::route_multichain_aggregator_service,
        multichain_aggregator_service_server::MultichainAggregatorServiceServer,
    },
    services::{HealthService, MultichainAggregator},
    settings::Settings,
};
use blockscout_service_launcher::{
    database,
    launcher::{self, LaunchSettings},
};
use migration::Migrator;
use multichain_aggregator_logic::{
    clients::{bens, dapp, token_info},
    services::chains::{
        fetch_and_upsert_blockscout_chains, start_marketplace_enabled_cache_updater,
    },
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

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

    let repo = database::ReadWriteRepo::new::<Migrator>(
        &settings.database,
        settings.replica_database.as_ref(),
    )
    .await?;

    if settings.service.fetch_chains {
        fetch_and_upsert_blockscout_chains(repo.main_db()).await?;
    }

    let dapp_client = dapp::new_client(settings.service.dapp_client.url)?;
    let token_info_client = token_info::new_client(settings.service.token_info_client.url)?;
    let bens_client = bens::new_client(settings.service.bens_client.url)?;

    let marketplace_enabled_cache = Arc::new(RwLock::new(HashMap::new()));
    start_marketplace_enabled_cache_updater(
        repo.read_db().clone(),
        dapp_client.clone(),
        marketplace_enabled_cache.clone(),
        settings.service.marketplace_enabled_cache_update_interval,
        settings.service.marketplace_enabled_cache_fetch_concurrency,
    );

    let multichain_aggregator = Arc::new(MultichainAggregator::new(
        repo,
        dapp_client,
        token_info_client,
        bens_client,
        settings.service.api,
        settings.service.quick_search_chains,
        settings.service.bens_protocols,
        marketplace_enabled_cache,
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
        graceful_shutdown: Default::default(),
    };

    launcher::launch(launch_settings, http_router, grpc_router).await
}
