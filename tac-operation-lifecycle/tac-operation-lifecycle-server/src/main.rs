use std::sync::Arc;

use blockscout_service_launcher::{database, launcher::ConfigSettings};
use migration::Migrator;
use tac_operation_lifecycle_logic::{client::Client, database::TacDatabase};
use tac_operation_lifecycle_server::{run, Settings};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");

    let db_connection = database::initialize_postgres::<Migrator>(&settings.database).await?;

    let db = Arc::new(TacDatabase::new(
        Arc::new(db_connection),
        settings.indexer.clone().unwrap().start_timestamp,
    ));

    let client = Arc::new(Mutex::new(Client::new(settings.clone().rpc)));

    run(settings, db.clone(), client).await
}
