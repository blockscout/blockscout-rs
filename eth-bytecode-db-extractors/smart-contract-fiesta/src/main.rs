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
    }

    let client = VerificationClient::try_new_arc(db_connection, settings.blockscout_url)
        .context("verification client initialization")?;

    client
        .verify_contracts()
        .await
        .context("verify contracts")?;

    Ok(())
}
