use crate::{
    proto::{
        database_actix::route_database, health_actix::route_health, health_server::HealthServer,
        solidity_verifier_actix::route_solidity_verifier,
        solidity_verifier_server::SolidityVerifierServer,
        sourcify_verifier_actix::route_sourcify_verifier,
        sourcify_verifier_server::SourcifyVerifierServer,
        vyper_verifier_actix::route_vyper_verifier, vyper_verifier_server::VyperVerifierServer,
    },
    services::{
        DatabaseService, HealthService, SolidityVerifierService, SourcifyVerifierService,
        VyperVerifierService,
    },
    settings::Settings,
};
use blockscout_service_launcher::LaunchSettings;
use std::sync::Arc;

#[derive(Clone)]
struct Router {
    database: Option<Arc<DatabaseService>>,
    solidity_verifier: Option<Arc<SolidityVerifierService>>,
    vyper_verifier: Option<Arc<VyperVerifierService>>,
    sourcify_verifier: Option<Arc<SourcifyVerifierService>>,

    health: Arc<HealthService>,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_optional_service(
                self.solidity_verifier
                    .clone()
                    .map(SolidityVerifierServer::from_arc),
            )
            .add_optional_service(
                self.vyper_verifier
                    .clone()
                    .map(VyperVerifierServer::from_arc),
            )
            .add_optional_service(
                self.sourcify_verifier
                    .clone()
                    .map(SourcifyVerifierServer::from_arc),
            )
    }
}

impl blockscout_service_launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));

        if let Some(database) = &self.database {
            service_config.configure(|config| route_database(config, database.clone()));
        }
        if let Some(solidity) = &self.solidity_verifier {
            service_config.configure(|config| route_solidity_verifier(config, solidity.clone()));
        }
        if let Some(vyper) = &self.vyper_verifier {
            service_config.configure(|config| route_vyper_verifier(config, vyper.clone()));
        }
        if let Some(sourcify) = &self.sourcify_verifier {
            service_config.configure(|config| route_sourcify_verifier(config, sourcify.clone()));
        }
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    let health = Arc::new(HealthService::default());
    let database = Arc::new(DatabaseService::default());
    let solidity_verifier = Arc::new(SolidityVerifierService::default());
    let vyper_verifier = Arc::new(VyperVerifierService::default());
    let sourcify_verifier = Arc::new(SourcifyVerifierService::default());

    let router = Router {
        database: Some(database),
        solidity_verifier: Some(solidity_verifier),
        vyper_verifier: Some(vyper_verifier),
        sourcify_verifier: Some(sourcify_verifier),
        health,
    };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let launch_settings = LaunchSettings {
        service_name: "eth-bytecode-db".to_owned(),
        server: settings.server,
        metrics: settings.metrics,
        jaeger: settings.jaeger,
    };
    blockscout_service_launcher::launch(&launch_settings, http_router, grpc_router).await
}
