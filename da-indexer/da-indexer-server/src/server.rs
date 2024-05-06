use crate::{
    proto::{health_actix::route_health, health_server::HealthServer},
    services::{DaService, HealthService},
    settings::Settings,
};
use blockscout_service_launcher::{launcher, launcher::LaunchSettings};

use da_indexer_proto::blockscout::da_indexer::v1::{
    da_service_actix::route_da_service, da_service_server::DaServiceServer,
};
use sea_orm::DatabaseConnection;

use std::sync::Arc;

const SERVICE_NAME: &str = "da_indexer";

#[derive(Clone)]
struct Router {
    health: Arc<HealthService>,
    da: Arc<DaService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(DaServiceServer::from_arc(self.da.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config.configure(|config| route_da_service(config, self.da.clone()));
    }
}

pub async fn run(
    settings: Settings,
    database_connection: DatabaseConnection,
) -> Result<(), anyhow::Error> {
    let health = Arc::new(HealthService::default());
    let da = Arc::new(DaService::new(database_connection.clone()));

    let router = Router { health, da };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };

    launcher::launch(&launch_settings, http_router, grpc_router).await
}
