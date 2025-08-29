use std::sync::Arc;

use blockscout_service_launcher::{database, launcher::ConfigSettings};
use migration::Migrator;
use zetachain_cctx_logic::client::Client;
use zetachain_cctx_server::{run, Settings};

#[actix_web::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    let db_connection = database::initialize_postgres::<Migrator>(&settings.database).await?;

    let db = Arc::new(db_connection);
    let client = Arc::new(Client::new(settings.rpc.clone()));
    run(settings, db, client).await
}
