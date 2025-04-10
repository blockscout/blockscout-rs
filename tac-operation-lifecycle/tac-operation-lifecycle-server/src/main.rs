use std::sync::Arc;

use blockscout_service_launcher::{database, launcher::ConfigSettings};
use migration::Migrator;
use tac_operation_lifecycle_logic::{client::Client, database::TacDatabase, Indexer};
use tac_operation_lifecycle_server::{run, Settings};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");

    let db_connection = database::initialize_postgres::<Migrator>(&settings.database).await?;

    let db = Arc::new(TacDatabase::new(Arc::new(db_connection)));

    let client = Arc::new(Mutex::new(Client::new(settings.rpc.clone())));

    let mut indexer = Indexer::new(settings.clone().indexer.unwrap(), db.clone()).await?;
    let realtime_boundary = indexer.realtime_boundary();

    tokio::spawn(async move {
        indexer.start(client).await.unwrap();
    });

    run(settings, db.clone(), realtime_boundary).await
}
