use std::sync::Arc;

use actix_phoenix_channel::ChannelCentral;
use anyhow::Context;
use blockscout_service_launcher::{database, launcher::ConfigSettings};
use migration::Migrator;
use zetachain_cctx_logic::{channel::Channel, client::Client};
use zetachain_cctx_server::{run, Settings};
#[actix_web::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    let db_connection = database::initialize_postgres::<Migrator>(&settings.database)
        .await
        .context("failed to initialize database")?;

    let db = Arc::new(db_connection);
    let client = Arc::new(Client::new(settings.rpc.clone()));
    let channel = Arc::new(ChannelCentral::new(Channel));
    run(settings, db, client, channel).await
}
