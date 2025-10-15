use crate::{
    proto::{
        health_actix::route_health, health_server::HealthServer,
    },
    services::{
        HealthService
    },
    settings::Settings,
};
use blockscout_service_launcher::{
    database,
    launcher, launcher::LaunchSettings, tracing};
use migration::Migrator;
use std::sync::Arc;
use crate::services::InterchainServiceImpl;
use crate::proto::interchain_service_server::InterchainServiceServer;
use crate::proto::interchain_service_actix::route_interchain_service;
const SERVICE_NAME: &str = "interchain_indexer";

#[derive(Clone)]
struct Router {
    // TODO: add services here
    health: Arc<HealthService>,
    interchain_service: Arc<InterchainServiceImpl>,
    }

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(InterchainServiceServer::from_arc(self.interchain_service.clone()))
            }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config.configure(|config| route_interchain_service(config, self.interchain_service.clone()));
        }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    let db = Arc::new(database::initialize_postgres::<Migrator>(&settings.database).await?);
    let interchain_service = Arc::new(InterchainServiceImpl {
        db: db.clone(),
        });
    let router = Router {
        health,
        interchain_service,
        };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
        graceful_shutdown: Default::default(),
    };

    launcher::launch(launch_settings, http_router, grpc_router).await
}
