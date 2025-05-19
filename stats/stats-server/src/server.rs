use std::{collections::HashMap, future::Future, path::PathBuf, sync::Arc, time::Duration};

use crate::{
    auth::{ApiKey, AuthorizationProvider},
    blockscout_waiter::{self, init_blockscout_api_client, IndexingStatusListener},
    config::{self, read_charts_config, read_layout_config, read_update_groups_config},
    health::HealthService,
    read_service::ReadService,
    runtime_setup::RuntimeSetup,
    settings::{
        handle_disable_internal_transactions, handle_enable_all_arbitrum,
        handle_enable_all_eip_7702, handle_enable_all_op_stack, Settings,
    },
    update_service::UpdateService,
};

use anyhow::{anyhow, Context};
use blockscout_endpoint_swagger::route_swagger;
use blockscout_service_launcher::{
    database::{DatabaseConnectOptionsSettings, DatabaseConnectSettings, DatabaseSettings},
    launcher::{self, GracefulShutdownHandler, LaunchSettings},
};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use stats::{data_source::types::BlockscoutMigrations, lines::NewBuilderAccounts, metrics, Named};
use stats_proto::blockscout::stats::v1::{
    health_actix::route_health,
    health_server::HealthServer,
    stats_service_actix::route_stats_service,
    stats_service_server::{StatsService, StatsServiceServer},
};
use tokio::task::JoinSet;
use tokio_util::task::TaskTracker;

const SERVICE_NAME: &str = "stats";

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
    handle_enable_all_op_stack(settings.enable_all_op_stack, &mut charts_config);
    handle_enable_all_eip_7702(settings.enable_all_eip_7702, &mut charts_config);
    handle_disable_internal_transactions(
        settings.disable_internal_transactions,
        &mut settings.conditional_start,
        &mut charts_config,
    );

    let charts = init_runtime_setup(charts_config, layout_config, update_groups_config)?;
    let db = init_stats_db(&settings).await?;
    let blockscout = connect_to_blockscout_db(&settings).await?;

    check_if_unsupported_charts_are_enabled(&charts, &blockscout).await?;
    create_charts_if_needed(&db, &charts).await?;

    if settings.metrics.enabled {
        metrics::initialize_metrics(charts.charts_info.keys().map(|f| f.as_str()));
    }

    let shutdown = shutdown.unwrap_or_default();
    let mut futures = JoinSet::new();

    let (status_waiter_task, status_listener) = init_waiter(&settings)?;
    if let Some(status_waiter_task) = status_waiter_task {
        spawn_and_track(&mut futures, &shutdown.task_tracker, status_waiter_task);
    }

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
    spawn_and_track(&mut futures, &shutdown.task_tracker, async move {
        update_service_cloned
            .run(
                settings.concurrent_start_updates,
                settings.default_schedule,
                settings.force_update_on_start,
            )
            .await;
        Ok(())
    });
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
        graceful_shutdown: shutdown.clone(),
    };
    spawn_and_track(&mut futures, &shutdown.task_tracker, async move {
        launcher::launch(launch_settings, http_router, grpc_router).await
    });
    let shutdown_cloned = shutdown.clone();
    spawn_and_track(&mut futures, &shutdown.task_tracker, async move {
        shutdown_cloned.shutdown_token.cancelled().await;
        Ok(())
    });

    let res = futures.join_next().await;
    on_termination(&db, &blockscout, &shutdown, &mut futures).await;
    res.expect("task set is not empty")?
}

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

fn spawn_and_track<F>(
    futures: &mut JoinSet<F::Output>,
    tracker: &TaskTracker,
    future: F,
) -> tokio::task::AbortHandle
where
    F: Future,
    F: Send + 'static,
    F::Output: Send,
{
    futures.spawn(tracker.track_future(future))
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

async fn connect_to_blockscout_db(settings: &Settings) -> anyhow::Result<Arc<DatabaseConnection>> {
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

async fn check_if_unsupported_charts_are_enabled(
    setup: &RuntimeSetup,
    blockscout_db: &DatabaseConnection,
) -> anyhow::Result<()> {
    let migrations = BlockscoutMigrations::query_from_db(blockscout_db).await?;
    if !migrations.denormalization {
        let charts_without_normalization = &[NewBuilderAccounts::name()];
        let mut all_enabled_charts_with_deps = setup.update_groups.values().flat_map(|g| {
            g.group
                .enabled_members_with_deps(&g.enabled_members)
                .into_iter()
        });
        if let Some(key) = all_enabled_charts_with_deps
            .find(|key| charts_without_normalization.contains(&key.name().to_string()))
        {
            return Err(anyhow!(
                "chart with name '{key}' is not supported without denormalized database. \
                Ensure denormalization is complete or disable the corresponding charts."
            ));
        }
    }
    Ok(())
}

async fn create_charts_if_needed(
    db: &DatabaseConnection,
    charts: &RuntimeSetup,
) -> anyhow::Result<()> {
    for group_entry in charts.update_groups.values() {
        group_entry
            .group
            .create_charts_sync(db, None, &group_entry.enabled_members)
            .await?;
    }
    Ok(())
}

/// Returns `(<waiter task>, <listener>)`
fn init_waiter(
    settings: &Settings,
) -> anyhow::Result<(
    Option<impl Future<Output = anyhow::Result<()>>>,
    Option<IndexingStatusListener>,
)> {
    let blockscout_api_config = init_blockscout_api_client(settings)?;
    let (status_waiter, status_listener) = blockscout_api_config
        .map(|c| blockscout_waiter::init(c, settings.conditional_start.clone()))
        .unzip();
    let status_task = status_waiter.map(|w| {
        async move {
            w.run().await?;
            // we don't want to finish on success because of the way
            // the tasks are handled here
            sleep_indefinitely().await;
            anyhow::Result::<()>::Ok(())
        }
    });
    Ok((status_task, status_listener))
}

fn init_authorization(api_keys: HashMap<String, String>) -> Arc<AuthorizationProvider> {
    if api_keys.is_empty() {
        tracing::warn!("No api keys found in settings, provide them to make use of authorization-protected endpoints")
    }
    let api_keys = api_keys
        .into_iter()
        .map(|(name, key)| (name, ApiKey::new(key)))
        .collect();
    Arc::new(AuthorizationProvider::new(api_keys))
}

async fn on_termination(
    db: &DatabaseConnection,
    blockscout: &DatabaseConnection,
    shutdown: &GracefulShutdownHandler,
    futures: &mut JoinSet<anyhow::Result<()>>,
) {
    if let Err(e) = db.close_by_ref().await {
        tracing::error!("Failed to close stats db connection upon termination: {e:?}");
    }
    if let Err(e) = blockscout.close_by_ref().await {
        tracing::error!("Failed to close blockscout db connection upon termination: {e:?}");
    }
    shutdown.shutdown_token.cancel();
    futures.abort_all();
}
