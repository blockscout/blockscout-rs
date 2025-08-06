use crate::{
    proto::{
        cluster_explorer_service_actix::route_cluster_explorer_service,
        cluster_explorer_service_server::ClusterExplorerServiceServer, health_actix::route_health,
        health_server::HealthServer,
        multichain_aggregator_service_actix::route_multichain_aggregator_service,
        multichain_aggregator_service_server::MultichainAggregatorServiceServer,
    },
    services::{ClusterExplorer, HealthService, MultichainAggregator},
    settings::Settings,
};
use actix_phoenix_channel::{ChannelCentral, configure_channel_websocket_route};
use blockscout_service_launcher::{
    database,
    launcher::{self, LaunchSettings},
};
use migration::Migrator;
use multichain_aggregator_logic::{
    clients::{bens, blockscout, dapp, token_info},
    metrics,
    services::{
        chains::{
            ChainSource, MarketplaceEnabledCache, fetch_and_upsert_blockscout_chains,
            list_active_chains_cached,
        },
        channel::Channel,
        cluster::Cluster,
        search::UniformChainSearchCache,
    },
};
use recache::stores::redis::RedisStore;
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::Duration,
};

const SERVICE_NAME: &str = "multichain_aggregator";

#[derive(Clone)]
struct Router {
    multichain_aggregator: Arc<MultichainAggregator>,
    cluster_explorer: Arc<ClusterExplorer>,
    health: Arc<HealthService>,
    channel: Arc<ChannelCentral<Channel>>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(MultichainAggregatorServiceServer::from_arc(
                self.multichain_aggregator.clone(),
            ))
            .add_service(ClusterExplorerServiceServer::from_arc(
                self.cluster_explorer.clone(),
            ))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config.configure(|config| {
            route_multichain_aggregator_service(config, self.multichain_aggregator.clone())
        });
        service_config.configure(|config| {
            route_cluster_explorer_service(config, self.cluster_explorer.clone())
        });
        service_config.configure(|config| {
            configure_channel_websocket_route(config, self.channel.clone());
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

    let marketplace_enabled_cache = MarketplaceEnabledCache::new();
    marketplace_enabled_cache.clone().start_updater(
        repo.read_db().clone(),
        dapp_client.clone(),
        settings.service.marketplace_enabled_cache_update_interval,
        settings.service.marketplace_enabled_cache_fetch_concurrency,
    );

    let channel = Arc::new(ChannelCentral::new(Channel));

    let uniform_chain_search_cache = if let Some(cache_settings) = settings.cache {
        let cache_name = "uniform_chain_search";
        let redis_cache = RedisStore::builder()
            .connection_string(cache_settings.redis.url.to_string())
            .reconnect_retry_factor(2)
            .reconnect_max_delay(Duration::from_secs(30))
            .prefix(format!("multichain_aggregator:{cache_name}"))
            .build()
            .await?;

        let cache_handler = UniformChainSearchCache::builder(Arc::new(redis_cache))
            .default_ttl(cache_settings.uniform_chain_search_cache.ttl)
            .maybe_default_refresh_ahead(cache_settings.uniform_chain_search_cache.refresh_ahead)
            .on_hit(Arc::new(|_, _| {
                metrics::CACHE_HIT_TOTAL
                    .with_label_values(&[cache_name])
                    .inc();
            }))
            .on_computed(Arc::new(|_, _| {
                // `on_computed` is triggered when the cache entry is computed and not found in the cache.
                // In this case, we consider it a cache miss.
                metrics::CACHE_MISS_TOTAL
                    .with_label_values(&[cache_name])
                    .inc();
            }))
            .on_refresh_computed(Arc::new(|_, _| {
                metrics::CACHE_REFRESH_AHEAD_TOTAL
                    .with_label_values(&[cache_name])
                    .inc();
            }))
            .build();

        Some(cache_handler)
    } else {
        None
    };

    let chain_urls = list_active_chains_cached(repo.read_db(), &[ChainSource::Repository])
        .await?
        .into_iter()
        .map(|c| (c.id, c.explorer_url))
        .collect::<BTreeMap<_, _>>();

    let clusters = settings
        .cluster_explorer
        .clusters
        .into_iter()
        .map(|(name, cluster)| {
            let chain_ids = cluster.chain_ids.into_iter().collect::<HashSet<_>>();
            if chain_ids.is_empty() {
                panic!("cluster {name} has no chain_ids");
            }
            let blockscout_clients = chain_ids
                .into_iter()
                .map(|id| {
                    let url = chain_urls
                        .get(&id)
                        .cloned()
                        .expect("chain should be present")
                        .expect("chain url should be present")
                        .parse()
                        .expect("chain url should be valid");
                    (
                        id,
                        blockscout::new_client(url).expect("client should be valid"),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            (name.clone(), Cluster::new(blockscout_clients))
        })
        .collect();
    let cluster_explorer = Arc::new(ClusterExplorer::new(
        repo.read_db().clone(),
        clusters,
        settings.service.api.clone(),
    ));

    let multichain_aggregator = Arc::new(MultichainAggregator::new(
        Arc::new(repo),
        dapp_client,
        token_info_client,
        bens_client,
        settings.service.api,
        settings.service.quick_search_chains,
        settings.service.bens_protocols,
        settings.service.domain_primary_chain_id,
        marketplace_enabled_cache,
        channel.channel_broadcaster(),
        uniform_chain_search_cache,
    ));

    let router = Router {
        health,
        multichain_aggregator,
        cluster_explorer,
        channel,
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
