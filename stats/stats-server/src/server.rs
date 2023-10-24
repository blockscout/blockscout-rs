use crate::{
    charts::Charts, config::read_charts_config, health::HealthService, read_service::ReadService,
    settings::Settings, update_service::UpdateService,
};
use blockscout_service_launcher::launcher::{self, LaunchSettings};
use sea_orm::{ConnectOptions, Database};
use stats_proto::blockscout::stats::v1::{
    health_actix::route_health,
    health_server::HealthServer,
    stats_service_actix::route_stats_service,
    stats_service_server::{StatsService, StatsServiceServer},
};
use std::sync::Arc;

const SERVICE_NAME: &str = "stats";

#[derive(Clone)]
struct HttpRouter<S: StatsService> {
    stats: Arc<S>,
    health: Arc<HealthService>,
}

impl<S: StatsService> launcher::HttpRouter for HttpRouter<S> {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config
            .configure(|config| route_health(config, self.health.clone()))
            .configure(|config| route_stats_service(config, self.stats.clone()));
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

pub async fn stats(settings: Settings) -> Result<(), anyhow::Error> {
    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &settings.tracing,
        &settings.jaeger,
    )?;
    let charts_config = read_charts_config(&settings.charts_config)?;
    let mut opt = ConnectOptions::new(settings.db_url.clone());
    opt.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    blockscout_service_launcher::database::initialize_postgres::<stats::migration::Migrator>(
        opt.clone(),
        settings.create_database,
        settings.run_migrations,
    )
    .await?;
    let db = Arc::new(Database::connect(opt).await?);

    let mut opt = ConnectOptions::new(settings.blockscout_db_url.clone());
    opt.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    let blockscout = Arc::new(Database::connect(opt).await?);

    let charts = Arc::new(Charts::new(charts_config)?);

    // TODO: may be run this with migrations or have special config
    for chart_info in charts.charts_info.values() {
        chart_info.chart.create(&db).await?;
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

    let read_service = Arc::new(ReadService::new(db, charts).await?);
    let health = Arc::new(HealthService::default());

    let grpc_router = grpc_router(read_service.clone(), health.clone());
    let http_router = HttpRouter {
        stats: read_service,
        health: health.clone(),
    };

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };

    launcher::launch(&launch_settings, http_router, grpc_router).await
}
