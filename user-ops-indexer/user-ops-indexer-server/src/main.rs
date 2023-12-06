use blockscout_service_launcher::database;
use blockscout_service_launcher::launcher::ConfigSettings;
use migration::Migrator;
use user_ops_indexer_server::{run_indexer, run_server, Settings};

const SERVICE_NAME: &str = "user_ops_indexer";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");

    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &settings.tracing,
        &settings.jaeger,
    )?;

    let db_connection = database::initialize_postgres::<Migrator>(
        &settings.database.url,
        settings.database.create_database,
        settings.database.run_migrations,
    )
    .await?;

    tokio::spawn(run_indexer(settings.clone(), db_connection.clone()));

    run_server(settings, db_connection).await
}
