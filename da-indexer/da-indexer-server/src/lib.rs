mod indexer;
mod proto;
mod server;
mod services;
mod settings;

pub use indexer::run as run_indexer;
pub use server::run as run_server;
pub use settings::Settings;

/********** run application **********/

use anyhow::Context;
use blockscout_service_launcher::database;
use da_indexer_logic::{celestia::l2_router::L2Router, s3_storage::S3Storage};
use migration::Migrator;

const SERVICE_NAME: &str = "da_indexer";

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &settings.tracing,
        &settings.jaeger,
    )?;

    let db_connection = match settings.database.clone() {
        Some(database_settings) => {
            Some(database::initialize_postgres::<Migrator>(&database_settings).await?)
        }
        None => None,
    };

    let s3_storage = match settings.s3_storage.clone() {
        Some(s3_storage_settings) => Some(
            S3Storage::new(s3_storage_settings)
                .await
                .context("s3 storage initialization failed")?,
        ),
        None => None,
    };

    let mut l2_router = None;
    if let Some(settings) = settings.l2_router.clone() {
        l2_router = Some(L2Router::from_settings(settings)?);
    }

    if let Some(indexer_settings) = settings.indexer.clone() {
        let db_connection = db_connection.expect("database is required for the indexer");
        run_indexer(indexer_settings, db_connection, s3_storage.clone()).await?;
    }

    let db_connection = match settings.database.clone() {
        Some(mut database_settings) => {
            database_settings.create_database = false;
            database_settings.run_migrations = false;
            Some(database::initialize_postgres::<Migrator>(&database_settings).await?)
        }
        None => None,
    };

    run_server(settings, db_connection, s3_storage, l2_router).await
}
