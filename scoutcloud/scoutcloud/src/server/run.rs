use crate::server::{
    proto::{
        health_actix::route_health, health_server::HealthServer, scoutcloud_actix::route_scoutcloud,
    },
    services::{HealthService, ScoutcloudService},
    settings::Settings,
};
use blockscout_service_launcher::{database, launcher, launcher::LaunchSettings, tracing};
use migration::Migrator;
use scoutcloud_proto::blockscout::scoutcloud::v1::scoutcloud_server::ScoutcloudServer;
use std::sync::Arc;

const SERVICE_NAME: &str = "scoutcloud";

#[derive(Clone)]
struct Router {
    // TODO: add services here
    health: Arc<HealthService>,
    scoutcloud: Arc<ScoutcloudService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(ScoutcloudServer::from_arc(self.scoutcloud.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config
            .configure(|config| route_health(config, self.health.clone()))
            .configure(|config| route_scoutcloud(config, self.scoutcloud.clone()));
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    let _db_connection = database::initialize_postgres::<Migrator>(
        &settings.database.url,
        settings.database.create_database,
        settings.database.run_migrations,
    )
    .await?;

    let scoutcloud = Arc::new(ScoutcloudService::default());

    let router = Router { health, scoutcloud };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };

    launcher::launch(&launch_settings, http_router, grpc_router).await
}
