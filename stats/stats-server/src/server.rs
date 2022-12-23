use crate::{service::Service, settings::Settings};
use actix_web::web::ServiceConfig;
use blockscout_service_launcher::LaunchSettings;
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
    let stats = Arc::new(Service::new(&settings.db_url, &settings.blockscout_db_url).await?);
    if settings.run_migrations {
        stats.migrate().await?;
    };

    let grpc_router = grpc_router(stats.clone());
    let http_router = HttpRouter { stats };
    let launch_settings = LaunchSettings {
        service_name: "stats".to_owned(),
        server: settings.server,
        metrics: settings.metrics,
        jaeger: settings.jaeger,
    };

    blockscout_service_launcher::launch(&launch_settings, http_router, grpc_router).await
}
