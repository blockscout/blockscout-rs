use crate::{
    create_provider_pools_from_chains, load_bridges_from_file, load_chains_from_file, proto::{
        health_actix::route_health, health_server::HealthServer,
        interchain_service_actix::route_interchain_service,
        interchain_service_server::InterchainServiceServer,
        interchain_statistics_service_server::InterchainStatisticsServiceServer,
    }, services::{HealthService, InterchainServiceImpl, InterchainStatisticsServiceImpl}, settings::Settings
};
use blockscout_service_launcher::{database, launcher, launcher::LaunchSettings, tracing as bs_tracing};
use interchain_indexer_entity::{bridge_contracts, bridges, chains};
use interchain_indexer_logic::{InterchainDatabase, crosschain_indexer::CrosschainIndexer, example::indexer::ExampleIndexer};
use interchain_indexer_proto::blockscout::interchain_indexer::v1::interchain_statistics_service_actix::route_interchain_statistics_service;
use migration::Migrator;
use std::sync::Arc;
use tracing;
const SERVICE_NAME: &str = "interchain_indexer";

#[derive(Clone)]
struct Router {
    // TODO: add services here
    health: Arc<HealthService>,
    interchain_service: Arc<InterchainServiceImpl>,
    stats_service: Arc<InterchainStatisticsServiceImpl>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(InterchainServiceServer::from_arc(
                self.interchain_service.clone(),
            ))
            .add_service(InterchainStatisticsServiceServer::from_arc(
                self.stats_service.clone(),
            ))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config
            .configure(|config| route_interchain_service(config, self.interchain_service.clone()));
        service_config.configure(|config| {
            route_interchain_statistics_service(config, self.stats_service.clone())
        });
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    bs_tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    let db_connection =
        Arc::new(database::initialize_postgres::<Migrator>(&settings.database).await?);
    let db = Arc::new(InterchainDatabase::new(db_connection));

    // Reading chains and bridges from json config files
    let chains = load_chains_from_file(&settings.chains_config).unwrap();
    let bridges = load_bridges_from_file(&settings.bridges_config).unwrap();

    // Populate database with the chains, bridges and bridge contracts
    db.upsert_chains(
        chains
            .clone()
            .into_iter()
            .map(chains::ActiveModel::from)
            .collect::<Vec<chains::ActiveModel>>(),
    )
    .await?;
    db.upsert_bridges(
        bridges
            .clone()
            .iter()
            .map(|b| bridges::ActiveModel::from(b.clone()))
            .collect::<Vec<bridges::ActiveModel>>(),
    )
    .await?;
    let bridge_contracts: Vec<bridge_contracts::ActiveModel> = bridges
        .iter()
        .flat_map(|bridge| {
            bridge
                .contracts
                .iter()
                .map(move |contract| contract.to_active_model(bridge.bridge_id))
        })
        .collect();
    if !bridge_contracts.is_empty() {
        db.upsert_bridge_contracts(bridge_contracts.clone()).await?;
    }

    tracing::info!(
        "Loaded {} chains ({}), {} bridges ({}) and {} bridge contracts from JSON files",
        chains.len(),
        chains
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<String>>()
            .join(", "),
        bridges.len(),
        bridges
            .iter()
            .map(|b| b.name.clone())
            .collect::<Vec<String>>()
            .join(", "),
        bridge_contracts.len(),
    );

    // Create layered provider objects for each chain
    // The provider for the chain is shared between all indexers on that chain
    let chains_providers = create_provider_pools_from_chains(chains.clone()).await?;

    bridges.iter().for_each(|bridge| {
        let indexer = match bridge.indexer.to_lowercase().as_str() {
            "avalanchenative" => {
                // TODO: start Avalanche indexer
                ExampleIndexer::new(
                    db.clone(),
                    bridge.bridge_id,
                    chains_providers.clone(),
                    settings.example_indexer.clone(),
                )
            }
            "omnibridge" => {
                // TODO: start OmniBridge indexer
                ExampleIndexer::new(
                    db.clone(),
                    bridge.bridge_id,
                    chains_providers.clone(),
                    settings.example_indexer.clone(),
                )
            }
            _ => {
                tracing::error!("Unsupported indexer: {}", bridge.indexer);
                return;
            }
        };

        if let Err(e) = indexer.unwrap().start_indexing() {
            tracing::error!("Failed to start indexing for bridge {}: {}", bridge.name, e);
        }
    });

    let interchain_service = Arc::new(InterchainServiceImpl::new(db.clone()));
    let stats_service = Arc::new(InterchainStatisticsServiceImpl::new(db.clone()));
    let router = Router {
        health,
        interchain_service,
        stats_service,
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
