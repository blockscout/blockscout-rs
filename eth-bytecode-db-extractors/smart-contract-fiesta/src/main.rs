use anyhow::Context;
use blockscout_service_launcher::{
    database, launcher::ConfigSettings, tracing as launcher_tracing,
};
use migration::Migrator;
use smart_contract_fiesta::{dataset, Settings, VerificationClient};
use std::sync::Arc;

const SERVICE_NAME: &str = "smart-contract-fiesta-extractor";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    launcher_tracing::init_logs(SERVICE_NAME, &Default::default(), &Default::default())
        .context("tracing initialization")?;

    let settings = Settings::build().context("failed to read config")?;

    let mut connect_options = sea_orm::ConnectOptions::new(&settings.database_url);
    connect_options.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    let db_connection = Arc::new(
        database::initialize_postgres::<Migrator>(
            connect_options,
            settings.create_database,
            settings.run_migrations,
        )
        .await?,
    );

    if settings.import_dataset {
        dataset::import_dataset(
            db_connection.clone(),
            settings
                .dataset
                .expect("validated in settings initialization"),
        )
        .await
        .context("dataset import")?;

        return Ok(());
    }

    let client = VerificationClient::try_new_arc(
        db_connection,
        settings.blockscout_url,
        settings.etherscan_url,
        settings.etherscan_api_key,
        settings.etherscan_limit_requests_per_second,
        settings.eth_bytecode_db_url,
    )
    .await
    .context("verification client initialization")?;

    if settings.search_enabled {
        for _ in 0..settings.search_n_threads {
            let client = client.clone();
            tokio::spawn(client.search_contracts());
        }
    }

    let mut handles = Vec::with_capacity(settings.n_threads);
    for _ in 0..settings.n_threads {
        let client = client.clone();
        let handle = tokio::spawn(client.verify_contracts());
        handles.push(handle);
    }
    for result in futures::future::join_all(handles).await {
        result.context("join handle")?.context("verify contracts")?;
    }

    Ok(())
}
