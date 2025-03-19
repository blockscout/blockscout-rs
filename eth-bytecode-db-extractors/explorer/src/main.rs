use anyhow::Context;
use blockscout_service_launcher::database;
use blockscout_service_launcher::launcher::ConfigSettings;
use explorer::Settings;

const SERVICE_NAME: &str = "explorer-extractor";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &Default::default(),
        &Default::default(),
    )
    .context("tracing initialization failed")?;

    let settings = Settings::build().context("failed to read config")?;

    let db_connection =
        database::initialize_postgres::<explorer_migration::Migrator>(&settings.database).await?;
    let explorer = explorer::Explorer::new(&settings)?;
    
    let client = explorer::Client::new(db_connection, explorer);

    let mut handles = vec![];
    for _ in 0..settings.n_threads {
        let client = client.clone();
        let handle = tokio::spawn(client.get_source_code());
        handles.push(handle);
    }
    for result in futures::future::join_all(handles).await {
        result.context("join handle")?.context("get source code")?;
    }

    Ok(())
}
