use crate::{
    charts::Charts, charts_config, health::HealthService, read_service::ReadService,
    settings::Settings, update_service::UpdateService,
};
use actix_web::web::ServiceConfig;
use blockscout_service_launcher::LaunchSettings;
use sea_orm::{ConnectOptions, Database};
use stats::migration::MigratorTrait;
use stats_proto::blockscout::stats::v1::{
    health_actix::route_health,
    health_server::HealthServer,
    stats_service_actix::route_stats_service,
    stats_service_server::{StatsService, StatsServiceServer},
};
use std::sync::Arc;

pub fn http_configure(config: &mut ServiceConfig, s: Arc<impl StatsService>) {
    route_stats_service(config, s);
}

#[derive(Clone)]
struct HttpRouter<S: StatsService> {
    stats: Arc<S>,
    health: Arc<HealthService>,
}

impl<S: StatsService> blockscout_service_launcher::HttpRouter for HttpRouter<S> {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config
            .configure(|config| route_health(config, self.health.clone()))
            .configure(|config| http_configure(config, self.stats.clone()));
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
    let launch_settings = LaunchSettings {
        service_name: "stats".to_owned(),
        server: settings.server,
        metrics: settings.metrics,
        tracing: settings.tracing,
        jaeger: settings.jaeger,
    };
    blockscout_service_launcher::init_logs(
        &launch_settings.service_name,
        &launch_settings.tracing,
        &launch_settings.jaeger,
    )?;

    let charts_config = std::fs::read(settings.charts_config)?;
    let charts_config: charts_config::Config = toml::from_slice(&charts_config)?;

    let mut opt = ConnectOptions::new(settings.db_url.clone());
    opt.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    let db = Arc::new(Database::connect(opt).await?);

    let mut opt = ConnectOptions::new(settings.blockscout_db_url.clone());
    opt.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    let blockscout = Arc::new(Database::connect(opt).await?);

    if settings.run_migrations {
        stats::migration::Migrator::up(&db, None).await?;
    }

    let charts = Arc::new(Charts::new(charts_config)?);

    // TODO: may be run this with migrations or have special config
    for chart in charts.charts.iter() {
        chart.create(&db).await?;
    }

    let update_service =
        Arc::new(UpdateService::new(db.clone(), blockscout, charts.clone()).await?);

    tokio::spawn(async move {
        if let Some(force_update) = settings.force_update_on_start {
            update_service
                .clone()
                .force_update_all_in_series(force_update)
                .await;
        }
        update_service.run(settings.default_schedule);
    });

    let read_service = Arc::new(ReadService::new(db, charts).await?);
    let health = Arc::new(HealthService::default());

    let grpc_router = grpc_router(read_service.clone(), health.clone());
    let http_router = HttpRouter {
        stats: read_service,
        health: health.clone(),
    };

    blockscout_service_launcher::launch(&launch_settings, http_router, grpc_router).await
}
