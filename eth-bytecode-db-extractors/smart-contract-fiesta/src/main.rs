use anyhow::Context;
use migration::Migrator;
use smart_contract_fiesta::{database, dataset, Settings, VerificationClient};
use std::sync::Arc;

const _SERVICE_NAME: &str = "smart-contract-fiesta-extractor";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::new().context("failed to read config")?;

    database::initialize_postgres::<Migrator>(
        &settings.database_url,
        settings.create_database,
        settings.run_migrations,
    )
    .await?;
    let db_connection = Arc::new(sea_orm::Database::connect(settings.database_url).await?);

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
