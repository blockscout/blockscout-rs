use crate::{
    proto::{health_actix::route_health, health_server::HealthServer},
    services::{CelestiaService, EigenDaService, HealthService},
    settings::Settings,
};
use blockscout_service_launcher::{launcher, launcher::LaunchSettings};

use da_indexer_logic::celestia::l2_router::L2Router;
use da_indexer_proto::blockscout::da_indexer::v1::{
    celestia_service_actix::route_celestia_service, celestia_service_server::CelestiaServiceServer,
    eigen_da_service_actix::route_eigen_da_service, eigen_da_service_server::EigenDaServiceServer,
};
use sea_orm::DatabaseConnection;

use std::sync::Arc;

const SERVICE_NAME: &str = "da_indexer";

#[derive(Clone)]
struct Router {
    health: Arc<HealthService>,
    celestia: Arc<CelestiaService>,
    eigenda: Arc<EigenDaService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(CelestiaServiceServer::from_arc(self.celestia.clone()))
            .add_service(EigenDaServiceServer::from_arc(self.eigenda.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config.configure(|config| route_celestia_service(config, self.celestia.clone()));
        service_config.configure(|config| route_eigen_da_service(config, self.eigenda.clone()));
    }
}

pub async fn run(
    settings: Settings,
    database_connection: Option<DatabaseConnection>,
    l2_router: Option<L2Router>,
) -> Result<(), anyhow::Error> {
    let health = Arc::new(HealthService::default());
    let celestia = Arc::new(CelestiaService::new(database_connection.clone(), l2_router));
    let eigenda = Arc::new(EigenDaService::new(database_connection.clone()));

    let router = Router {
        health,
        celestia,
        eigenda,
    };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };

    launcher::launch(&launch_settings, http_router, grpc_router).await
}
