use crate::{
    proto::{
        health_actix::route_health, health_server::HealthServer,
        solidity_visualizer_actix::route_solidity_visualizer,
        solidity_visualizer_server::SolidityVisualizerServer,
    },
    services::{HealthService, SolidityVisualizerService},
    settings::Settings,
};
use blockscout_service_launcher::{launcher, launcher::LaunchSettings, tracing};
use std::sync::Arc;

const SERVICE_NAME: &str = "visualizer";

#[derive(Clone)]
struct Router {
    visualizer: Arc<SolidityVisualizerService>,
    health: Arc<HealthService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(SolidityVisualizerServer::from_arc(self.visualizer.clone()))
            .add_service(HealthServer::from_arc(self.health.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config
            .configure(|config| route_solidity_visualizer(config, self.visualizer.clone()));
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let visualizer = Arc::new(SolidityVisualizerService::default());
    let health = Arc::new(HealthService::default());

    let router = Router { visualizer, health };
    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_owned(),
        server: settings.server,
        metrics: settings.metrics,
    };
    launcher::launch(&launch_settings, http_router, grpc_router).await
}
