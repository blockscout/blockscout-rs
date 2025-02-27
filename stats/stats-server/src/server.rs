use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use crate::{
    auth::{ApiKey, AuthorizationProvider},
    blockscout_waiter::{self, init_blockscout_api_client, IndexingStatusListener},
    config::{self, read_charts_config, read_layout_config, read_update_groups_config},
    health::HealthService,
    read_service::ReadService,
    runtime_setup::RuntimeSetup,
    settings::{handle_disable_internal_transactions, handle_enable_all_arbitrum, Settings},
    update_service::UpdateService,
};

use anyhow::Context;
use blockscout_endpoint_swagger::route_swagger;
use blockscout_service_launcher::{
    database::{DatabaseConnectOptionsSettings, DatabaseConnectSettings, DatabaseSettings},
    launcher::{self, GracefulShutdownHandler, LaunchSettings},
};
use futures::FutureExt;
use itertools::Itertools;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use stats::metrics;
use stats_proto::blockscout::stats::v1::{
    health_actix::route_health,
    health_server::HealthServer,
    stats_service_actix::route_stats_service,
    stats_service_server::{StatsService, StatsServiceServer},
};
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;

const SERVICE_NAME: &str = "stats";

#[derive(Clone)]
struct HttpRouter<S: StatsService> {
    stats: Arc<S>,
    health: Arc<HealthService>,
    swagger_path: PathBuf,
}

impl<S: StatsService> launcher::HttpRouter for HttpRouter<S> {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config
            .configure(|config| route_health(config, self.health.clone()))
            .configure(|config| route_stats_service(config, self.stats.clone()))
            .configure(|config| {
                route_swagger(
                    config,
                    self.swagger_path.clone(),
                    // it's ok to not have this endpoint in swagger, as it is
                    // the swagger itself
                    "/api/v1/docs/swagger.yaml",
                )
            });
    }
}

fn grpc_router<S: StatsService>(
    stats: Arc<S>,
    health: Arc<HealthService>,
) -> tonic::transport::server::Router {
    tonic::transport::Server::builder()
        .add_service(HealthServer::from_arc(health))
        .add_service(StatsServiceServer::from_arc(stats))
}

async fn sleep_indefinitely() {
    tokio::time::sleep(Duration::from_secs(u64::MAX)).await;
}

async fn init_stats_db(settings: &Settings) -> anyhow::Result<Arc<DatabaseConnection>> {
    let database_settings = DatabaseSettings {
        connect: DatabaseConnectSettings::Url(settings.db_url.clone()),
        connect_options: DatabaseConnectOptionsSettings::default(),
        create_database: settings.create_database,
        run_migrations: settings.run_migrations,
    };
    let db = Arc::new(
        blockscout_service_launcher::database::initialize_postgres::<stats::migration::Migrator>(
            &database_settings,
        )
        .await
        .context("stats DB")?,
    );
    Ok(db)
}

async fn init_blockscout_db(settings: &Settings) -> anyhow::Result<Arc<DatabaseConnection>> {
    let mut opt = ConnectOptions::new(settings.blockscout_db_url.clone());
    opt.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    // we'd like to have each batch to resolve in under 1 hour
    // as it seems to be the middleground between too many steps & occupying DB for too long
    opt.sqlx_slow_statements_logging_settings(
        tracing::log::LevelFilter::Warn,
        Duration::from_secs(3600),
    );
    let conn = Arc::new(Database::connect(opt).await.context("blockscout DB")?);
    Ok(conn)
}

fn init_runtime_setup(
    charts_config: config::charts::Config<config::types::AllChartSettings>,
    layout_config: config::layout::Config,
    update_groups_config: config::update_groups::Config,
) -> anyhow::Result<Arc<RuntimeSetup>> {
    let setup = RuntimeSetup::new(charts_config, layout_config, update_groups_config)?;
    Ok(Arc::new(setup))
}

async fn create_charts_if_needed(
    db: &DatabaseConnection,
    charts: &RuntimeSetup,
) -> anyhow::Result<()> {
    // TODO: maybe run this with migrations or have special config
    for group_entry in charts.update_groups.values() {
        group_entry
            .group
            .create_charts_sync(db, None, &group_entry.enabled_members)
            .await?;
    }
    Ok(())
}

type WaiterHandle = JoinHandle<Result<(), anyhow::Error>>;

/// Returns `(<waiter spawned task join handle>, <listener>)`
fn init_and_spawn_waiter(
    tracker: &TaskTracker,
    settings: &Settings,
) -> anyhow::Result<(Option<WaiterHandle>, Option<IndexingStatusListener>)> {
    let blockscout_api_config = init_blockscout_api_client(settings)?;
    let (status_waiter, status_listener) = blockscout_api_config
        .map(|c| blockscout_waiter::init(c, settings.conditional_start.clone()))
        .unzip();
    let status_waiter_handle = status_waiter.map(|w| {
        tracker.spawn(async move {
            w.run().await?;
            // we don't want to finish on success because of the way
            // the tasks are handled here
            sleep_indefinitely().await;
            Ok(())
        })
    });
    Ok((status_waiter_handle, status_listener))
}

fn init_authorization(api_keys: HashMap<String, ApiKey>) -> Arc<AuthorizationProvider> {
    if api_keys.is_empty() {
        tracing::warn!("No api keys found in settings, provide them to make use of authorization-protected endpoints")
    }
    Arc::new(AuthorizationProvider::new(api_keys))
}

pub async fn stats(
    mut settings: Settings,
    shutdown: Option<GracefulShutdownHandler>,
) -> Result<(), anyhow::Error> {
    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &settings.tracing,
        &settings.jaeger,
    )?;
    let mut charts_config = read_charts_config(&settings.charts_config)?;
    let layout_config = read_layout_config(&settings.layout_config)?;
    let update_groups_config = read_update_groups_config(&settings.update_groups_config)?;
    handle_enable_all_arbitrum(settings.enable_all_arbitrum, &mut charts_config);
    handle_disable_internal_transactions(
        settings.disable_internal_transactions,
        &mut settings.conditional_start,
        &mut charts_config,
    );

    let charts = init_runtime_setup(charts_config, layout_config, update_groups_config)?;
    let db = init_stats_db(&settings).await?;
    let blockscout = init_blockscout_db(&settings).await?;

    create_charts_if_needed(&db, &charts).await?;

    let shutdown = shutdown.unwrap_or(GracefulShutdownHandler::new());
    let (status_waiter_handle, status_listener) =
        init_and_spawn_waiter(&shutdown.task_tracker, &settings)?;

    let update_service = Arc::new(
        UpdateService::new(
            db.clone(),
            blockscout.clone(),
            charts.clone(),
            status_listener,
        )
        .await?,
    );

    let update_service_cloned = update_service.clone();
    let update_service_handle = shutdown.task_tracker.spawn(async move {
        update_service_cloned
            .run(
                settings.concurrent_start_updates,
                settings.default_schedule,
                settings.force_update_on_start,
            )
            .await;
        Ok(())
    });

    if settings.metrics.enabled {
        metrics::initialize_metrics(charts.charts_info.keys().map(|f| f.as_str()));
    }
    let authorization = init_authorization(settings.api_keys);

    let read_service = Arc::new(
        ReadService::new(
            db.clone(),
            blockscout.clone(),
            charts,
            update_service,
            authorization,
            settings.limits.into(),
        )
        .await?,
    );

    let health = Arc::new(HealthService::default());
    let grpc_router = grpc_router(read_service.clone(), health.clone());
    let http_router = HttpRouter {
        stats: read_service,
        health: health.clone(),
        swagger_path: settings.swagger_file,
    };
    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };
    let shutdown_cloned = shutdown.clone();
    let servers_handle = shutdown.task_tracker.spawn(async move {
        launcher::launch(&launch_settings, http_router, grpc_router, shutdown_cloned).await
    });

    let mut spawned = vec![update_service_handle, servers_handle];
    if let Some(status_waiter_handle) = status_waiter_handle {
        spawned.push(status_waiter_handle);
    }
    let futures = [async move {
        shutdown.shutdown_token.cancelled().await;
        Ok(Ok(()))
    }]
    .into_iter()
    .map(|t| t.boxed());

    let to_abort = spawned
        .iter()
        .map(|join_handle| join_handle.abort_handle())
        .collect_vec();
    let all_tasks = spawned
        .into_iter()
        .map(|join| join.boxed())
        .chain(futures)
        .collect_vec();
    let (res, _, _) = futures::future::select_all(all_tasks).await;
    db.get_postgres_connection_pool().close().await;
    blockscout.get_postgres_connection_pool().close().await;
    for a in to_abort.into_iter() {
        a.abort()
    }
    res?
}
