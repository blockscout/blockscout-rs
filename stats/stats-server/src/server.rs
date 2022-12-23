use crate::{read_service::ReadService, settings::Settings, update_service::UpdateService};
use actix_web::{web::ServiceConfig, App, HttpServer};
use sea_orm::Database;
use stats::{migration::MigratorTrait, Chart, NewBlocks, TotalBlocks};
use stats_proto::blockscout::stats::v1::{
    stats_service_actix::route_stats_service,
    stats_service_server::{StatsService, StatsServiceServer},
};
use std::{net::SocketAddr, sync::Arc};

pub fn http_configure(config: &mut ServiceConfig, s: Arc<impl StatsService>) {
    route_stats_service(config, s);
}

pub fn http_server(s: Arc<impl StatsService>, addr: SocketAddr) -> actix_web::dev::Server {
    tracing::info!("starting http server on addr {}", addr);
    let server =
        HttpServer::new(move || App::new().configure(|config| http_configure(config, s.clone())))
            .bind(addr)
            .unwrap_or_else(|_| panic!("failed to bind server"));

    server.run()
}

pub fn grpc_server(
    s: Arc<impl StatsService>,
    addr: SocketAddr,
) -> impl futures::Future<Output = Result<(), tonic::transport::Error>> {
    tracing::info!("starting grpc server on addr {}", addr);
    let server = tonic::transport::Server::builder().add_service(StatsServiceServer::from_arc(s));
    server.serve(addr)
}

pub async fn stats(settings: Settings) -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();

    let mut futures = vec![];

    let db = Arc::new(Database::connect(&settings.db_url).await?);
    let blockscout = Arc::new(Database::connect(&settings.blockscout_db_url).await?);

    if settings.run_migrations {
        stats::migration::Migrator::up(&db, None).await?;
    }

    let charts: Vec<Arc<dyn Chart + Send + Sync + 'static>> = vec![
        Arc::new(TotalBlocks::default()),
        Arc::new(NewBlocks::default()),
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

    if settings.server.http.enabled {
        let http_server = {
            let http_server_future = http_server(read_service.clone(), settings.server.http.addr);
            tokio::spawn(async move { http_server_future.await.map_err(anyhow::Error::msg) })
        };
        futures.push(http_server)
    }

    if settings.server.grpc.enabled {
        let grpc_server = {
            let grpc_server_future = grpc_server(read_service.clone(), settings.server.grpc.addr);
            tokio::spawn(async move { grpc_server_future.await.map_err(anyhow::Error::msg) })
        };
        futures.push(grpc_server)
    }

    let (res, _, others) = futures::future::select_all(futures).await;
    for future in others.into_iter() {
        future.abort()
    }
    res?
}
