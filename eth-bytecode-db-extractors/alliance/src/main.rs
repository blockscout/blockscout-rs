mod client;
mod proxy_verifier;
mod settings;

use alliance::{Client, Settings};
use anyhow::Context;
use blockscout_service_launcher::{self as launcher, launcher::ConfigSettings};
use migration::Migrator;

const SERVICE_NAME: &str = "alliance-extractor";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    launcher::tracing::init_logs(SERVICE_NAME, &Default::default(), &Default::default())
        .context("tracing initialization")?;

    let settings = Settings::build().context("failed to read config")?;

    let mut connect_options = sea_orm::ConnectOptions::new(&settings.database_url);
    connect_options.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    let db_connection =
        launcher::database::initialize_postgres::<Migrator>(connect_options, false, false).await?;

    let client = Client::try_new(db_connection, settings.proxy_verifier_url)?;

    let mut handles = Vec::with_capacity(settings.n_threads);
    for _ in 0..settings.n_threads {
        let client = client.clone();
        let handle = tokio::spawn(client.extract());
        handles.push(handle);
    }
    for result in futures::future::join_all(handles).await {
        result.context("join handle")?.context("verify contracts")?;
    }

    Ok(())
}
