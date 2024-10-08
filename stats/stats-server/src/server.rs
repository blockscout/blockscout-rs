use std::{path::PathBuf, sync::Arc, time::Duration};

use crate::{
    config::{read_charts_config, read_layout_config, read_update_groups_config},
    health::HealthService,
    read_service::ReadService,
    runtime_setup::RuntimeSetup,
    settings::{Settings, StartConditionSettings},
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
use tokio::time::sleep;
use tracing::info;

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

fn is_threshold_passed(
    threshold: Option<f32>,
    float_value: Option<String>,
    value_name: &str,
) -> Result<bool, anyhow::Error> {
    let Some(threshold) = threshold else {
        return Ok(true);
    };
    let value = float_value
        .map(|s| s.parse::<f64>())
        .transpose()
        .context(format!("Parsing `{value_name}`"))?;
    let Some(value) = value else {
        anyhow::bail!("Received `null` value of `{value_name}`. Can't determine indexing status.",);
    };
    if value < threshold.into() {
        info!(
            threshold = threshold,
            current_value = value,
            "Threshold for `{value_name}` is not satisfied"
        );
        Ok(false)
    } else {
        info!(
            threshold = threshold,
            current_value = value,
            "Threshold for `{value_name}` is satisfied"
        );
        Ok(true)
    }
}

async fn wait_for_blockscout_indexing(
    api_config: blockscout_client::Configuration,
    wait_config: StartConditionSettings,
) -> Result<(), anyhow::Error> {
    loop {
        let result = blockscout_client::apis::main_page_api::get_indexing_status(&api_config)
            .await
            .context("Requesting indexing status")?;
        if !is_threshold_passed(
            wait_config.blocks_ratio_threshold,
            result.indexed_blocks_ratio,
            "indexed_blocks_ratio",
        )? || !is_threshold_passed(
            wait_config.internal_transactions_ratio_threshold,
            result.indexed_internal_transactions_ratio,
            "indexed_internal_transactions_ratio",
        )? {
            info!(
                "Blockscout is not indexed enough. Checking again in {} secs",
                wait_config.check_period_secs
            );
            sleep(Duration::from_secs(wait_config.check_period_secs.into())).await;
        }
    }
}

pub async fn stats(settings: Settings) -> Result<(), anyhow::Error> {
    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &settings.tracing,
        &settings.jaeger,
    )?;
    let charts_config = read_charts_config(&settings.charts_config)?;
    let layout_config = read_layout_config(&settings.layout_config)?;
    let update_groups_config = read_update_groups_config(&settings.update_groups_config)?;
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

    // Wait for blockscout to index, if necessary.
    if let Some(api_config) = settings.api_url.map(blockscout_client::Configuration::new) {
        wait_for_blockscout_indexing(api_config, settings.conditional_start).await?;
    }

    let update_service =
        Arc::new(UpdateService::new(db.clone(), blockscout, charts.clone()).await?);

    tokio::spawn(async move {
        update_service
            .force_async_update_and_run(
                settings.concurrent_start_updates,
                settings.default_schedule,
                settings.force_update_on_start,
            )
            .await;
    });

    if settings.metrics.enabled {
        metrics::initialize_metrics(charts.charts_info.keys().map(|f| f.as_str()));
    }

    let read_service = Arc::new(ReadService::new(db, charts, settings.limits.into()).await?);
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

    launcher::launch(&launch_settings, http_router, grpc_router).await
}
