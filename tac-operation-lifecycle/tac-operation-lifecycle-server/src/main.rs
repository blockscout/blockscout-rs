use std::sync::Arc;

use blockscout_service_launcher::launcher::ConfigSettings;
use migration::Migrator;
use tac_operation_lifecycle_server::{Settings, run};
use tac_operation_lifecycle_logic::Indexer;
use blockscout_service_launcher::database;
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {

    let settings = Settings::build().expect("failed to read config");
    

    let db_connection = database::initialize_postgres::<Migrator>(
        &settings.database,
    )
    .await?;
    
    let indexer = Indexer::new(settings.clone().indexer.unwrap(), Arc::new(db_connection)).await?;

    tokio::spawn(async move {
        indexer.start().await.unwrap();
    });
    run(settings).await
}
