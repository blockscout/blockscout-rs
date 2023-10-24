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
use blockscout_service_launcher::{database, launcher, launcher::LaunchSettings, tracing};
use eth_bytecode_db::verification::Client;
use migration::Migrator;
use std::{collections::HashSet, sync::Arc};

const SERVICE_NAME: &str = "eth_bytecode_db";

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

impl launcher::HttpRouter for Router {
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
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    let db_connection = database::initialize_postgres::<Migrator>(
        &settings.database.url,
        settings.database.create_database,
        settings.database.run_migrations,
    )
    .await?;

    let mut client = Client::new(db_connection, settings.verifier.uri).await?;
    if settings.verifier_alliance_database.enabled {
        let alliance_db_connection =
            sea_orm::Database::connect(settings.verifier_alliance_database.url).await?;
        client = client.with_alliance_db(alliance_db_connection);
    }

    let sourcify_client = sourcify::ClientBuilder::default()
        .try_base_url(&settings.sourcify.base_url)
        .map_err(|err| anyhow::anyhow!(err))?
        .max_retries(settings.sourcify.max_retries)
        .build();
    let database = Arc::new(DatabaseService::new_arc(client.clone(), sourcify_client));

    let authorized_keys: HashSet<_> = settings
        .authorized_keys
        .into_values()
        .map(|key| key.key)
        .collect();

    let solidity_verifier = Arc::new(
        SolidityVerifierService::new(client.clone()).with_authorized_keys(authorized_keys.clone()),
    );
    let vyper_verifier =
        Arc::new(VyperVerifierService::new(client.clone()).with_authorized_keys(authorized_keys));
    let sourcify_verifier = Arc::new(SourcifyVerifierService::new(client.clone()));

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
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };

    launcher::launch(&launch_settings, http_router, grpc_router).await
}
