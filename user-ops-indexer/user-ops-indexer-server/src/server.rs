use crate::{
    proto::{
        health_actix::route_health, health_server::HealthServer,
        user_ops_service_actix::route_user_ops_service,
        user_ops_service_server::UserOpsServiceServer,
    },
    services::{HealthService, UserOpsService},
    settings::Settings,
};
use blockscout_endpoint_swagger::route_swagger;
use blockscout_service_launcher::{launcher, launcher::LaunchSettings};
use sea_orm::DatabaseConnection;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use user_ops_indexer_logic::indexer::status::IndexerStatus;

const SERVICE_NAME: &str = "user_ops_indexer_server";

#[derive(Clone)]
struct Router {
    health: Arc<HealthService>,
    user_ops: Arc<UserOpsService>,
    swagger_path: PathBuf,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(UserOpsServiceServer::from_arc(self.user_ops.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config.configure(|config| route_user_ops_service(config, self.user_ops.clone()));
        service_config.configure(|config| {
            route_swagger(
                config,
                self.swagger_path.clone(),
                "/api/v1/docs/swagger.yaml",
            )
        });
    }
}

pub async fn run(
    settings: Settings,
    database_connection: DatabaseConnection,
    status: Arc<RwLock<IndexerStatus>>,
) -> Result<(), anyhow::Error> {
    let health = Arc::new(HealthService::default());
    let user_ops = Arc::new(UserOpsService::new(
        database_connection,
        settings.api,
        status,
    ));

    let router = Router {
        health,
        user_ops,
        swagger_path: settings.swagger_path,
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
