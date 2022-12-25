use crate::{read_service::ReadService, settings::Settings, update_service::UpdateService};
use actix_web::web::ServiceConfig;
use blockscout_service_launcher::LaunchSettings;
use sea_orm::Database;
use stats::{counters, lines, migration::MigratorTrait, Chart};
use stats_proto::blockscout::stats::v1::{
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
}

impl<S: StatsService> blockscout_service_launcher::HttpRouter for HttpRouter<S> {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| http_configure(config, self.stats.clone()));
    }
}

fn grpc_router<S: StatsService>(stats: Arc<S>) -> tonic::transport::server::Router {
    tonic::transport::Server::builder().add_service(StatsServiceServer::from_arc(stats))
}

pub async fn stats(settings: Settings) -> Result<(), anyhow::Error> {
    let db = Arc::new(Database::connect(&settings.db_url).await?);
    let blockscout = Arc::new(Database::connect(&settings.blockscout_db_url).await?);

    if settings.run_migrations {
        stats::migration::Migrator::up(&db, None).await?;
    }

    let charts: Vec<Arc<dyn Chart + Send + Sync + 'static>> = vec![
        Arc::new(lines::NewBlocks::default()),
        Arc::new(counters::AverageBlockTime::default()),
        Arc::new(counters::CompletedTxns::default()),
        Arc::new(counters::TotalAccounts::default()),
        Arc::new(counters::TotalBlocks::default()),
        Arc::new(counters::TotalCoinHolders::default()),
        Arc::new(counters::TotalCoinTransfers::default()),
        Arc::new(counters::TotalTokens::default()),
        Arc::new(counters::TotalTxns::default()),
    ];
    // TODO: may be run this with migrations or have special config
    for chart in charts.iter() {
        chart.create(&db).await?;
    }

    let update_service = Arc::new(UpdateService::new(db.clone(), blockscout, charts).await?);
    tokio::spawn(async move {
        update_service.update().await;
        update_service.run_cron(settings.update_schedule).await;
    });

    let read_service = Arc::new(ReadService::new(db).await?);

    let grpc_router = grpc_router(read_service.clone());
    let http_router = HttpRouter {
        stats: read_service,
    };
    let launch_settings = LaunchSettings {
        service_name: "stats".to_owned(),
        server: settings.server,
        metrics: settings.metrics,
        jaeger: settings.jaeger,
    };

    blockscout_service_launcher::launch(&launch_settings, http_router, grpc_router).await
}
