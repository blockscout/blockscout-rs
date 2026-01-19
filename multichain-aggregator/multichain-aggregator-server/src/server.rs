use crate::{
    proto::{
        cluster_explorer_service_actix::route_cluster_explorer_service,
        cluster_explorer_service_server::ClusterExplorerServiceServer, health_actix::route_health,
        health_server::HealthServer,
        multichain_aggregator_service_actix::route_multichain_aggregator_service,
        multichain_aggregator_service_server::MultichainAggregatorServiceServer,
    },
    services::{ClusterExplorer, HealthService, MULTICHAIN_CLUSTER_ID, MultichainAggregator},
    settings::Settings,
};
use actix_phoenix_channel::{ChannelCentral, configure_channel_websocket_route};
use blockscout_service_launcher::{
    database,
    launcher::{self, LaunchSettings},
};
use migration::Migrator;
use multichain_aggregator_logic::{
    clients::{bens, blockscout, dapp},
    metrics,
    services::{
        chains::{
            ChainSource, MarketplaceEnabledCache, fetch_and_upsert_blockscout_chains,
            list_active_chains_cached,
        },
        channel::Channel,
        cluster::{
            Cluster, DecodedCalldataCache, DomainInfoCache, DomainProtocolsCache,
            DomainSearchCache, TokenSearchCache,
        },
        coin_price::build_coin_price_cache,
    },
};
use recache::stores::redis::RedisStore;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
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

macro_rules! add_cache_metrics {
    ($cache_builder:expr, $cache_name:expr) => {
        $cache_builder
        .on_hit(Arc::new(|_, _| {
            metrics::CACHE_HIT_TOTAL
                .with_label_values(&[$cache_name])
                .inc();
        }))
        .on_computed(Arc::new(|_, _| {
            // `on_computed` is triggered when the cache entry is computed and not found in the cache.
            // In this case, we consider it a cache miss.
            metrics::CACHE_MISS_TOTAL
                .with_label_values(&[$cache_name])
                .inc();
        }))
        .on_refresh_computed(Arc::new(|_, _| {
            metrics::CACHE_REFRESH_AHEAD_TOTAL
                .with_label_values(&[$cache_name])
                .inc();
        }))
    };
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
    let bens_client = bens::new_client(settings.service.bens_client.url)?;

    let marketplace_enabled_cache = MarketplaceEnabledCache::new();
    marketplace_enabled_cache.clone().start_updater(
        repo.read_db().clone(),
        dapp_client.clone(),
        settings.service.marketplace_enabled_cache_update_interval,
        settings.service.marketplace_enabled_cache_fetch_concurrency,
    );

    let channel = Arc::new(ChannelCentral::new(Channel));

    let redis_cache = if let Some(cache_settings) = &settings.cache {
        let redis_cache = RedisStore::builder()
            .connection_string(cache_settings.redis.url.to_string())
            .reconnect_retry_factor(2)
            .reconnect_max_delay(Duration::from_secs(30))
            .prefix("multichain_aggregator")
            .build()
            .await?;

        Some(Arc::new(redis_cache))
    } else {
        None
    };

    macro_rules! build_cache {
        ($settings:expr, $cache_type:ident, $cache_id:ident, $cache_name:expr) => {
            if let Some(cache_settings) = &$settings.cache
                && cache_settings.$cache_id.enabled
                && let Some(redis_cache) = &redis_cache
            {
                let cache_builder = $cache_type::builder(Arc::clone(redis_cache))
                    .default_ttl(cache_settings.$cache_id.ttl)
                    .maybe_default_refresh_ahead(cache_settings.$cache_id.refresh_ahead);
                let cache = add_cache_metrics!(cache_builder, $cache_name).build();
                Some(cache)
            } else {
                None
            }
        };
    }

    let domain_search_cache = build_cache!(
        settings,
        DomainSearchCache,
        domain_search_cache,
        "domain_search"
    );
    let domain_info_cache =
        build_cache!(settings, DomainInfoCache, domain_info_cache, "domain_info");
    let domain_protocols_cache = build_cache!(
        settings,
        DomainProtocolsCache,
        domain_protocols_cache,
        "domain_protocols"
    );
    let decoded_calldata_cache = build_cache!(
        settings,
        DecodedCalldataCache,
        decoded_calldata_cache,
        "decoded_calldata"
    );
    let token_search_cache = build_cache!(
        settings,
        TokenSearchCache,
        token_search_cache,
        "token_search"
    );

    let chain_urls = list_active_chains_cached(repo.read_db(), &[ChainSource::Repository])
        .await?
        .into_iter()
        .map(|c| (c.id, c.explorer_url))
        .collect::<BTreeMap<_, _>>();

    let mut clusters = settings
        .cluster_explorer
        .clusters
        .into_iter()
        .map(|(name, cluster)| {
            let chain_ids = cluster
                .chain_ids
                .into_iter()
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if chain_ids.is_empty() {
                panic!("cluster {name} has no chain_ids");
            }
            let blockscout_clients = chain_ids
                .iter()
                .map(|id| {
                    let url = chain_urls
                        .get(id)
                        .cloned()
                        .expect("chain should be present")
                        .expect("chain url should be present")
                        .parse()
                        .expect("chain url should be valid");
                    let client = blockscout::new_client(url).expect("client should be valid");
                    (*id, Arc::new(client))
                })
                .collect::<BTreeMap<_, _>>();
            let coin_price_cache = redis_cache.as_ref().cloned().map(build_coin_price_cache);

            (
                name.clone(),
                Cluster::new(
                    repo.read_db().clone(),
                    name,
                    chain_ids,
                    Arc::new(blockscout_clients),
                    decoded_calldata_cache.clone(),
                    settings.service.quick_search_chains.clone(),
                    dapp_client.clone(),
                    bens_client.clone(),
                    cluster.bens_priority_protocols,
                    domain_search_cache.clone(),
                    domain_info_cache.clone(),
                    domain_protocols_cache.clone(),
                    token_search_cache.clone(),
                    marketplace_enabled_cache.clone(),
                    coin_price_cache.clone(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    clusters.insert(
        MULTICHAIN_CLUSTER_ID.to_string(),
        Cluster::new(
            repo.read_db().clone(),
            MULTICHAIN_CLUSTER_ID.to_string(),
            Default::default(),
            Default::default(),
            decoded_calldata_cache.clone(),
            settings.service.quick_search_chains.clone(),
            dapp_client.clone(),
            bens_client.clone(),
            Default::default(),
            domain_search_cache.clone(),
            domain_info_cache.clone(),
            domain_protocols_cache.clone(),
            token_search_cache.clone(),
            marketplace_enabled_cache.clone(),
            None,
        ),
    );
    let cluster_explorer = Arc::new(ClusterExplorer::new(clusters, settings.service.api.clone()));

    let multichain_aggregator = Arc::new(MultichainAggregator::new(
        Arc::new(repo),
        dapp_client,
        settings.service.api,
        marketplace_enabled_cache,
        channel.channel_broadcaster(),
        Arc::clone(&cluster_explorer),
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
