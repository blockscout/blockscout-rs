use crate::{
    proto::{
        health_actix::route_health, health_server::HealthServer,
        tac_service_actix::route_tac_service,
        tac_statistic_actix::route_tac_statistic,
    },
    services::{
        HealthService, OperationsService, StatisticService
    },
    settings::Settings,
};
use blockscout_service_launcher::{
    launcher, launcher::LaunchSettings, tracing};


use tac_operation_lifecycle_logic::database::TacDatabase;

use std::sync::Arc;

const SERVICE_NAME: &str = "tac_operation_lifecycle";

#[derive(Clone)]
struct Router {
    // TODO: add services here
    health: Arc<HealthService>,
    stat: Arc<StatisticService>,
    operations: Arc<OperationsService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            //.add_service(StatisticService::from_arc(self.stat.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config.configure(|config| route_tac_service(config, self.operations.clone()));
        service_config.configure(|config| route_tac_statistic(config, self.stat.clone()));
    }
}

pub async fn run(settings: Settings, db: Arc<TacDatabase>, realtime_boundary: u64) -> Result<(), anyhow::Error> {
    println!("running server");
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());
    let stat = Arc::new(StatisticService::new(db.clone(), realtime_boundary));
    let operations = Arc::new(OperationsService::new(db.clone()));
    

    // TODO: init services here

    let router = Router {
        health,
        stat,
        operations,
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
