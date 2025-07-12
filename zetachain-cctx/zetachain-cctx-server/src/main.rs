use std::sync::Arc;

use blockscout_service_launcher::{database, launcher::ConfigSettings};
use zetachain_cctx_logic::client::Client;
use zetachain_cctx_server::{Settings, run};
use migration::Migrator;
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    let db_connection = database::initialize_postgres::<Migrator>(&settings.database).await?;

    let db = Arc::new(
        db_connection
    );
    let client = Arc::new(Client::new(settings.rpc.clone()));
    run(settings, db, client).await
}
