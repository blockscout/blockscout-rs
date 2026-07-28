// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{
    create_provider_pools_from_chains, load_bridges_from_file, load_chains_from_file,
    proto::{
        health_actix::route_health, health_server::HealthServer,
        interchain_service_actix::route_interchain_service,
        interchain_service_server::InterchainServiceServer,
        interchain_statistics_service_server::InterchainStatisticsServiceServer,
        status_service_server::StatusServiceServer,
    },
    services::{
        HealthService, InterchainServiceImpl, InterchainStatisticsServiceImpl, StatusServiceImpl,
    },
    settings::Settings,
    spawn_configured_indexers,
};
use blockscout_endpoint_swagger::route_swagger;
use blockscout_service_launcher::{
    database, launcher, launcher::LaunchSettings, tracing as bs_tracing,
};
use interchain_indexer_entity::{bridge_contracts, bridges, chains};
use interchain_indexer_logic::{
    ChainInfoService, IndexedChains, InterchainDatabase, StatsReadSettings, StatsService,
    TokenInfoService,
};
use interchain_indexer_proto::blockscout::interchain_indexer::v1::{
    interchain_statistics_service_actix::route_interchain_statistics_service,
    status_service_actix::route_status_service,
};
use migration::Migrator;
use std::{path::PathBuf, sync::Arc, time::Duration};
const SERVICE_NAME: &str = "interchain_indexer";

/// Spawns a Tokio task that recomputes `stats_chains` on a fixed interval.
///
/// The **first** recomputation runs immediately after startup wiring (before the first sleep),
/// so fresh stats are available without waiting a full period. Subsequent runs wait
/// `period_secs` after each attempt (success or failure). If `period_secs` is `0`, does nothing.
fn spawn_stats_chains_recalculation_worker(stats: Arc<StatsService>, period_secs: u64) {
    if period_secs == 0 {
        tracing::info!("stats_chains_recalculation_period_secs is 0: periodic refresh disabled");
        return;
    }

    tokio::spawn(async move {
        loop {
            tracing::info!("stats_chains recomputation started");
            match stats.recompute_stats_chains().await {
                Ok(()) => tracing::info!("stats_chains recomputation succeeded"),
                Err(err) => tracing::error!(
                    err = ?err,
                    "stats_chains recomputation failed; keeping previous rows, retrying after interval"
                ),
            }
            tokio::time::sleep(Duration::from_secs(period_secs)).await;
        }
    });
}

#[derive(Clone)]
struct Router {
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

    let chains = load_chains_from_file(&settings.chains_config)?;
    // Single provider pool per chain, shared by the token info service and the
    // indexers. `DynProvider` is `Arc`-backed, so cloning the map shares the
    // same underlying failover state (one health task, one primary per chain).
    let chain_providers = create_provider_pools_from_chains(chains.clone()).await?;
    let token_info_service = Arc::new(TokenInfoService::new(
        db.clone(),
        chain_providers.clone(),
        settings.token_info.clone(),
    ));
    // `load_bridges_from_file` is pure file IO and is hoisted above stats
    // wiring so the indexed-chain set below can be built from it. Nothing else
    // moves: the startup backfill a few lines down still runs before
    // `upsert_chains` / `upsert_bridges` / `upsert_bridge_contracts`, because a
    // DB-derived indexed-chain set would be stale exactly when backfill needs
    // it (the DB has no bridge_contracts rows yet on a fresh deployment).
    let bridges = load_bridges_from_file(&settings.bridges_config)?;

    // Stats eligibility ("can missing evidence still arrive?") is derived from
    // this in-memory set, never from `bridge_contracts`. Include *all*
    // declared bridges, even disabled ones: `enabled` is an operational
    // switch, not a statement about observability, and a contract-less bridge
    // must end up present in the map with an empty set (restrictive), not
    // omitted (permissive) — see `IndexedChains::may_observe`.
    let indexed_chains = IndexedChains::from_bridges(bridges.iter().map(|b| {
        (
            b.bridge_id,
            b.contracts.iter().map(|c| c.chain_id).collect(),
        )
    }));
    // Iterate the map (not the config) so a `chain_count = 0` line proves the
    // bridge is *present* in the map, not merely declared in the config file.
    for (bridge_id, chain_count) in indexed_chains.bridge_chain_counts() {
        tracing::info!(bridge_id, chain_count, "stats indexed-chain set");
        if chain_count == 0 {
            tracing::warn!(
                bridge_id,
                "bridge declared with no configured contracts; every chain counts as unindexed for it and its partial data will be committed to stats"
            );
        }
    }
    indexed_chains.record_metrics();
    // Misconfiguration guard, not a safety net: with bridges configured but
    // zero total pairs, every bridge in the map has an empty set, so every
    // chain is unindexed and every pending transfer becomes countable.
    anyhow::ensure!(
        bridges.is_empty() || indexed_chains.pair_count() > 0,
        "stats indexed-chain set is empty while {} bridge(s) are configured in {}: every \
         chain would be treated as unindexed for every bridge and every pending transfer \
         would become countable",
        bridges.len(),
        settings.bridges_config.display()
    );

    let stats = Arc::new(StatsService::new(
        db.clone(),
        Some(token_info_service.clone()),
        StatsReadSettings {
            include_zero_chains: settings.stats.include_zero_chains,
        },
        indexed_chains,
    ));

    if settings.stats.backfill_on_start {
        tracing::info!(
            "stats.backfill_on_start enabled: running statistics projection; async token enrichment will run after each batch outside DB transactions"
        );
        stats
            .backfill_stats_until_idle_with_token_enrichment()
            .await?;
    }

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
        "Loaded {} chains ({}), {} bridges ({}) and {} bridge contracts from \
         JSON files",
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

    let chain_info_service = Arc::new(ChainInfoService::new(
        db.clone(),
        settings.chain_info.clone(),
    ));

    let indexers = spawn_configured_indexers(
        stats.clone(),
        &bridges,
        &chains,
        &chain_providers,
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

    let api_settings = settings.api.clone();
    let interchain_service = Arc::new(InterchainServiceImpl::new(
        db.clone(),
        token_info_service.clone(),
        chain_info_service.clone(),
        bridges,
        api_settings.clone(),
    ));
    let stats_service = Arc::new(InterchainStatisticsServiceImpl::new(
        stats.clone(),
        api_settings,
        chain_info_service.clone(),
    ));
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

    let stats_chains_period_secs = settings.stats.chains_recalculation_period_secs;
    spawn_stats_chains_recalculation_worker(stats.clone(), stats_chains_period_secs);

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
