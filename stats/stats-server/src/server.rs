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
    let charts_config = std::fs::read(settings.charts_config)?;
    let charts_config = toml::from_slice(&charts_config)?;

    let db = Arc::new(Database::connect(&settings.db_url).await?);
    let blockscout = Arc::new(Database::connect(&settings.blockscout_db_url).await?);

    if settings.run_migrations {
        stats::migration::Migrator::up(&db, None).await?;
    }

    let charts: Vec<Arc<dyn Chart + Send + Sync + 'static>> = vec![
        // finished counters
        Arc::new(counters::TotalBlocks::default()),
        // finished lines
        Arc::new(lines::NewBlocks::default()),
        // mock counters
        Arc::new(counters::MockCounterDouble::new(
            "averageBlockTime".into(),
            34.25,
        )),
        Arc::new(counters::MockCounterInt::new(
            "completedTransactions".into(),
            956276037263,
        )),
        Arc::new(counters::MockCounterInt::new(
            "totalAccounts".into(),
            765543,
        )),
        Arc::new(counters::MockCounterInt::new(
            "totalNativeCoinHolders".into(),
            409559,
        )),
        Arc::new(counters::MockCounterInt::new(
            "totalNativeCoinTransfers".into(),
            32528,
        )),
        Arc::new(counters::MockCounterInt::new("totalTokens".into(), 1234)),
        Arc::new(counters::MockCounterInt::new(
            "totalTransactions".into(),
            84273733,
        )),
        // mock lines
        Arc::new(lines::MockLineInt::new("accountsGrowth".into(), 100..500)),
        Arc::new(lines::MockLineInt::new(
            "activeAccounts".into(),
            200..200_000,
        )),
        Arc::new(lines::MockLineInt::new(
            "averageBlockSize".into(),
            90_000..100_000,
        )),
        Arc::new(lines::MockLineInt::new(
            "averageGasLimit".into(),
            8_000_000..30_000_000,
        )),
        Arc::new(lines::MockLineDouble::new(
            "averageGasPrice".into(),
            5.0..200.0,
        )),
        Arc::new(lines::MockLineDouble::new(
            "averageTxnFee".into(),
            0.0001..0.01,
        )),
        Arc::new(lines::MockLineInt::new(
            "gasUsedGrowth".into(),
            1_000_000..100_000_000,
        )),
        Arc::new(lines::MockLineInt::new(
            "nativeCoinHoldersGrowth".into(),
            1000..5000,
        )),
        Arc::new(lines::MockLineInt::new(
            "nativeCoinSupply".into(),
            1_000_000..100_000_000,
        )),
        Arc::new(lines::MockLineInt::new(
            "newNativeCoinTransfers".into(),
            100..10_000,
        )),
        Arc::new(lines::MockLineInt::new("newTxns".into(), 200..20_000)),
        Arc::new(lines::MockLineDouble::new("txnsFee".into(), 0.0001..0.01)),
        Arc::new(lines::MockLineInt::new(
            "txnsGrowth".into(),
            1000..10_000_000,
        )),
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

    let read_service = Arc::new(ReadService::new(db, charts_config).await?);

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
