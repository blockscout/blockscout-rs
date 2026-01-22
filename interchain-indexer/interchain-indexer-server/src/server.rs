use crate::{
    ChainConfig, create_provider_pools_from_chains, load_bridges_from_file, load_chains_from_file, proto::{
        health_actix::route_health, health_server::HealthServer,
        interchain_service_actix::route_interchain_service,
        interchain_service_server::InterchainServiceServer,
        interchain_statistics_service_server::InterchainStatisticsServiceServer,
        status_service_server::StatusServiceServer,
    }, services::{
        HealthService, InterchainServiceImpl, InterchainStatisticsServiceImpl, StatusServiceImpl,
    }, settings::Settings, spawn_configured_indexers
};
use anyhow::{Context, bail};
use blockscout_endpoint_swagger::route_swagger;
use blockscout_service_launcher::{
    database, launcher, launcher::LaunchSettings, tracing as bs_tracing,
};
use interchain_indexer_entity::{bridge_contracts, bridges, chains};
use interchain_indexer_logic::{InterchainDatabase, TokenInfoService};
use interchain_indexer_proto::blockscout::interchain_indexer::v1::{
    interchain_statistics_service_actix::route_interchain_statistics_service,
    status_service_actix::route_status_service,
};
use migration::Migrator;
use std::{path::PathBuf, sync::Arc};
const SERVICE_NAME: &str = "interchain_indexer";

#[derive(Clone)]
struct Router {
    // TODO: add services here
    health: Arc<HealthService>,
    interchain_service: Arc<InterchainServiceImpl>,
    stats_service: Arc<InterchainStatisticsServiceImpl>,
    status_service: Arc<StatusServiceImpl>,
    swagger_path: PathBuf,
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
            .add_service(StatusServiceServer::from_arc(self.status_service.clone()))
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
        service_config
            .configure(|config| route_status_service(config, self.status_service.clone()));
        service_config.configure(|config| {
            route_swagger(
                config,
                self.swagger_path.clone(),
                "/api/v1/docs/swagger.yaml",
            )
        });
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    bs_tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    let db_connection =
        Arc::new(database::initialize_postgres::<Migrator>(&settings.database).await?);
    let interchain_db = InterchainDatabase::new(db_connection);
    let db = Arc::new(interchain_db.clone());

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
    prefill_avalanche_blockchain_ids(db.as_ref(), &chains).await?;
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

    let chains_providers = create_provider_pools_from_chains(chains.clone()).await?;

    let token_info_service = Arc::new(TokenInfoService::new(
        db.clone(),
        chains_providers.clone(),
        settings.token_info.clone(),
    ));

    let indexers = spawn_configured_indexers(
        interchain_db.clone(),
        &bridges,
        &chains,
        &chains_providers,
        &settings,
    )
    .await?;

    // let example = ExampleIndexer::new(
    //     db.clone(),
    //     bridges[0].bridge_id,
    //     chains_providers,
    //     Default::default(),
    // )?;

    // example.start_indexing().await?;

    let interchain_service = Arc::new(InterchainServiceImpl::new(
        db.clone(),
        token_info_service.clone(),
        bridges,
        settings.api,
    ));
    let stats_service = Arc::new(InterchainStatisticsServiceImpl::new(db.clone()));
    let status_service = Arc::new(StatusServiceImpl::new(indexers.clone()));
    let router = Router {
        health,
        interchain_service,
        stats_service,
        status_service,
        swagger_path: settings.swagger_path,
    };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
        graceful_shutdown: Default::default(),
    };

    let launch_result = launcher::launch(launch_settings, http_router, grpc_router).await;

    for indexer in indexers {
        indexer.stop().await;
    }

    //example.stop_indexing().await;

    launch_result
}

fn decode_blockchain_id(native_id: &str) -> anyhow::Result<Vec<u8>> {
    let trimmed = native_id.trim();
    let native_id = trimmed.trim_start_matches("0x");
    let bytes = hex::decode(native_id)
        .with_context(|| format!("native_id must be hex: {native_id}"))?;

    if bytes.len() != 32 {
        bail!("native_id must be 32 bytes, got {}", bytes.len());
    }

    Ok(bytes)
}

async fn prefill_avalanche_blockchain_ids(
    db: &InterchainDatabase,
    chains: &[ChainConfig],
) -> anyhow::Result<()> {
    for chain in chains {
        let Some(native_id) = chain
            .native_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            continue;
        };

        let blockchain_id = decode_blockchain_id(native_id).with_context(|| {
            format!(
                "invalid native_id configured for chain {} ({native_id})",
                chain.chain_id
            )
        })?;

        db.upsert_avalanche_icm_blockchain_id(blockchain_id, chain.chain_id)
            .await
            .with_context(|| {
                format!(
                    "failed to persist avalanche_icm_blockchain_ids mapping for chain {}",
                    chain.chain_id
                )
            })?;

        tracing::debug!(chain_id = chain.chain_id, "prefilled Avalanche blockchain id mapping");
    }

    Ok(())
}
