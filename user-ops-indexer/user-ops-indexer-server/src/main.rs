use blockscout_service_launcher::{database, launcher::ConfigSettings};
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

    let database_url = settings.database.connect.clone().url();
    let mut connect_options = sea_orm::ConnectOptions::new(&database_url);
    connect_options.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    let db_connection = database::initialize_postgres::<Migrator>(&settings.database).await?;

    let status = run_indexer(settings.clone(), db_connection).await?;

    let db_connection = database::initialize_postgres::<Migrator>(&settings.database).await?;

    run_server(settings, db_connection, status).await
}
