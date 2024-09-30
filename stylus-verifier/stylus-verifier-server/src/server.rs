use crate::{
    services::{health::HealthService, stylus_sdk_rs_verifier::StylusSdkRsVerifierService},
    settings::Settings,
};
use blockscout_service_launcher::{launcher, launcher::LaunchSettings, tracing};
use std::sync::Arc;
use stylus_verifier_proto::{
    blockscout::stylus_verifier::v1::{
        stylus_sdk_rs_verifier_actix::route_stylus_sdk_rs_verifier,
        stylus_sdk_rs_verifier_server::StylusSdkRsVerifierServer,
    },
    grpc::health::v1::{health_actix::route_health, health_server::HealthServer},
};

const SERVICE_NAME: &str = "stylus_verifier";

#[derive(Clone)]
struct Router {
    health: Arc<HealthService>,
    stylus_sdk_rs_verifier: Arc<StylusSdkRsVerifierService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(StylusSdkRsVerifierServer::from_arc(
                self.stylus_sdk_rs_verifier.clone(),
            ))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config.configure(|config| {
            route_stylus_sdk_rs_verifier(config, self.stylus_sdk_rs_verifier.clone())
        });
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());
    let stylus_sdk_rs_verifier = Arc::new(StylusSdkRsVerifierService::new(settings.docker_api));

    // TODO: init services here

    let router = Router {
        health,
        stylus_sdk_rs_verifier,
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
