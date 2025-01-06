use std::{path::PathBuf, sync::Arc, time::Duration};

use crate::{
    blockscout_waiter::{self, init_blockscout_api_client},
    config::{read_charts_config, read_layout_config, read_update_groups_config},
    health::HealthService,
    read_service::ReadService,
    runtime_setup::RuntimeSetup,
    settings::{handle_disable_internal_transactions, handle_enable_all_arbitrum, Settings},
    update_service::UpdateService,
};

use anyhow::Context;
use blockscout_endpoint_swagger::route_swagger;
use blockscout_service_launcher::launcher::{self, LaunchSettings};
use sea_orm::{ConnectOptions, Database};
use stats::metrics;
use stats_proto::blockscout::stats::v1::{
    health_actix::route_health,
    health_server::HealthServer,
    stats_service_actix::route_stats_service,
    stats_service_server::{StatsService, StatsServiceServer},
};

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

pub async fn stats(mut settings: Settings) -> Result<(), anyhow::Error> {
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
    let mut opt = ConnectOptions::new(settings.db_url.clone());
    opt.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    blockscout_service_launcher::database::initialize_postgres::<stats::migration::Migrator>(
        opt.clone(),
        settings.create_database,
        settings.run_migrations,
    )
    .await?;
    let db = Arc::new(Database::connect(opt).await.context("stats DB")?);

    let mut opt = ConnectOptions::new(settings.blockscout_db_url.clone());
    opt.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    // we'd like to have each batch to resolve in under 1 hour
    // as it seems to be the middleground between too many steps & occupying DB for too long
    opt.sqlx_slow_statements_logging_settings(
        tracing::log::LevelFilter::Warn,
        Duration::from_secs(3600),
    );
    let blockscout = Arc::new(Database::connect(opt).await.context("blockscout DB")?);

    let charts = Arc::new(RuntimeSetup::new(
        charts_config,
        layout_config,
        update_groups_config,
    )?);

    // TODO: maybe run this with migrations or have special config
    for group_entry in charts.update_groups.values() {
        group_entry
            .group
            .create_charts_with_mutexes(&db, None, &group_entry.enabled_members)
            .await?;
    }

    let blockscout_api_config = init_blockscout_api_client(&settings).await?;

    let (waiter, listener) = blockscout_api_config
        .clone()
        .map(|c| blockscout_waiter::init(c, settings.conditional_start.clone()))
        .unzip();
    let waiter_handle = waiter.map(|w| tokio::spawn(async move { w.run().await }));

    let update_service = Arc::new(
        UpdateService::new(db.clone(), blockscout.clone(), charts.clone(), listener).await?,
    );

    let update_service_handle = tokio::spawn(async move {
        update_service
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

    let read_service =
        Arc::new(ReadService::new(db, blockscout, charts, settings.limits.into()).await?);
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

    let mut futures = vec![
        update_service_handle,
        tokio::spawn(
            async move { launcher::launch(&launch_settings, http_router, grpc_router).await },
        ),
    ];
    if let Some(waiter_handle) = waiter_handle {
        futures.push(waiter_handle);
    }
    let (res, _, others) = futures::future::select_all(futures).await;
    for future in others.into_iter() {
        future.abort()
    }
    res?
}
