use crate::{
    proto::{
        database_actix::route_database, health_actix::route_health, health_server::HealthServer,
        solidity_verifier_actix::route_solidity_verifier,
        solidity_verifier_server::SolidityVerifierServer,
        sourcify_verifier_actix::route_sourcify_verifier,
        sourcify_verifier_server::SourcifyVerifierServer,
        verifier_alliance_server::VerifierAllianceServer,
        vyper_verifier_actix::route_vyper_verifier, vyper_verifier_server::VyperVerifierServer,
    },
    services::{
        DatabaseService, HealthService, SolidityVerifierService, SourcifyVerifierService,
        VerifierAllianceService, VyperVerifierService,
    },
    settings::Settings,
};
use anyhow::Context;
use blockscout_service_launcher::{database, launcher, launcher::LaunchSettings, tracing};
use eth_bytecode_db::verification::Client;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2::verifier_alliance_actix::route_verifier_alliance;
use migration::Migrator;
use std::{collections::HashSet, sync::Arc};

const SERVICE_NAME: &str = "eth_bytecode_db";

#[derive(Clone)]
struct Router {
    database: Option<Arc<DatabaseService>>,
    solidity_verifier: Option<Arc<SolidityVerifierService>>,
    vyper_verifier: Option<Arc<VyperVerifierService>>,
    sourcify_verifier: Option<Arc<SourcifyVerifierService>>,
    verifier_alliance: Option<Arc<VerifierAllianceService>>,

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
            .add_optional_service(
                self.verifier_alliance
                    .clone()
                    .map(VerifierAllianceServer::from_arc),
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
        if let Some(verifier_alliance) = &self.verifier_alliance {
            service_config
                .configure(|config| route_verifier_alliance(config, verifier_alliance.clone()));
        }
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    let db_connection = {
        let database_settings = database::DatabaseSettings {
            connect: database::DatabaseConnectSettings::Url(settings.database.url),
            connect_options: database::DatabaseConnectOptionsSettings {
                sqlx_logging_level: ::tracing::log::LevelFilter::Debug,
                ..Default::default()
            },
            create_database: settings.database.create_database,
            run_migrations: settings.database.run_migrations,
        };

        database::initialize_postgres::<Migrator>(&database_settings).await?
    };

    let mut client = Client::new(
        db_connection,
        settings.verifier.http_url.to_string(),
        settings.verifier.max_retries,
        settings.verifier.probe_url,
    )
    .await?;
    if settings.verifier_alliance_database.enabled {
        let alliance_db_main_settings = database::DatabaseSettings {
            connect: database::DatabaseConnectSettings::Url(
                settings.verifier_alliance_database.url,
            ),
            connect_options: database::DatabaseConnectOptionsSettings {
                sqlx_logging_level: ::tracing::log::LevelFilter::Debug,
                ..Default::default()
            },
            // Important!!!: never try to create verifier alliance database or run migrations on it,
            // as the database is shared between different explorers and is managed from outside.
            create_database: false,
            run_migrations: false,
        };

        // As no migrations should actually be made, we can use noop migrator which does nothing.
        let alliance_db_read_write_repo = database::ReadWriteRepo::new::<noop_migrator::Migrator>(
            &alliance_db_main_settings,
            settings.verifier_alliance_replica_database.as_ref(),
        )
        .await
        .context("alliance db read-write repo initialization")?;

        client = client.with_alliance_db(alliance_db_read_write_repo);
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
    let vyper_verifier = Arc::new(
        VyperVerifierService::new(client.clone()).with_authorized_keys(authorized_keys.clone()),
    );
    let sourcify_verifier = Arc::new(SourcifyVerifierService::new(client.clone()));

    let verifier_alliance = Arc::new(
        VerifierAllianceService::new(client.clone()).with_authorized_keys(authorized_keys),
    );

    let router = Router {
        database: Some(database),
        solidity_verifier: Some(solidity_verifier),
        vyper_verifier: Some(vyper_verifier),
        sourcify_verifier: Some(sourcify_verifier),
        verifier_alliance: Some(verifier_alliance),
        health,
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

// May be moved to `blockscout_service_launcher::database` later
mod noop_migrator {
    use migration::{MigrationTrait, MigratorTrait};

    pub struct Migrator;
    #[async_trait::async_trait]
    impl MigratorTrait for Migrator {
        fn migrations() -> Vec<Box<dyn MigrationTrait>> {
            vec![]
        }
    }
}
