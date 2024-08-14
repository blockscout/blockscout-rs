use blockscout_service_launcher::{database, launcher::ConfigSettings};
use da_indexer_logic::celestia::l2_router::L2Router;
use da_indexer_server::{run_indexer, run_server, Settings};
use migration::Migrator;

const SERVICE_NAME: &str = "da_indexer";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");

    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &settings.tracing,
        &settings.jaeger,
    )?;

    let db_connection = match settings.database.clone() {
        Some(database_settings) => {
            let database_url = database_settings.connect.clone().url();
            let mut connect_options = sea_orm::ConnectOptions::new(&database_url);
            connect_options.sqlx_logging_level(tracing::log::LevelFilter::Debug);
            Some(
                database::initialize_postgres::<Migrator>(
                    connect_options,
                    database_settings.create_database,
                    database_settings.run_migrations,
                )
                .await?,
            )
        }
        None => None,
    };

    let mut l2_router = None;
    if let Some(settings) = settings.l2_router.clone() {
        l2_router = Some(L2Router::from_settings(settings)?);
    }

    if let Some(indexer_settings) = settings.indexer.clone() {
        let db_connection = db_connection.expect("database is required for the indexer");
        run_indexer(indexer_settings, db_connection).await?;
    }

    let db_connection = match settings.database.clone() {
        Some(database_settings) => Some(
            database::initialize_postgres::<Migrator>(
                &database_settings.connect.clone().url(),
                false,
                false,
            )
            .await?,
        ),
        None => None,
    };

    run_server(settings, db_connection, l2_router).await
}
