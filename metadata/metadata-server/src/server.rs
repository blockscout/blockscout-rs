use crate::{
    proto::{
        health_actix::route_health, health_server::HealthServer, metadata_actix::route_metadata,
        metadata_server::MetadataServer,
    },
    services::{HealthService, MetadataService},
    settings::Settings,
};
use blockscout_service_launcher::{database, launcher, launcher::LaunchSettings, tracing};

use migration::Migrator;

use std::sync::Arc;

const SERVICE_NAME: &str = "metadata";

#[derive(Clone)]
struct Router {
    metadata: Arc<MetadataService>,

    health: Arc<HealthService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(MetadataServer::from_arc(self.metadata.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config.configure(|config| route_metadata(config, self.metadata.clone()));
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    let db_connection = database::initialize_postgres::<Migrator>(
        &settings.database.url,
        settings.database.create_database,
        settings.database.run_migrations,
    )
    .await?;

    let metadata = Arc::new(MetadataService::new(db_connection));

    let router = Router { metadata, health };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };

    launcher::launch(&launch_settings, http_router, grpc_router).await
}
