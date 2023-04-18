use extractors::{pending_addresses, Client};
use migration::Migrator;
use std::str::FromStr;
use tracing::log::LevelFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let chain = "xdai_mainnet";

    blockscout_service_launcher::init_logs(
        &format!("{chain} blockscout_extractor"),
        &blockscout_service_launcher::TracingSettings {
            enabled: true,
            format: blockscout_service_launcher::TracingFormat::Default,
        },
        &blockscout_service_launcher::JaegerSettings::default(),
    )
    .expect("logs initialization failed");

    let db_url = format!("postgres://postgres:admin@localhost:21432/{chain}");
    let db_conn_opt = sea_orm::ConnectOptions::new(db_url)
        .sqlx_logging_level(LevelFilter::Debug)
        .to_owned();
    blockscout_service_launcher::database::initialize_postgres::<Migrator>(
        db_conn_opt.clone(),
        true,
        true,
    )
    .await?;
    let db = sea_orm::Database::connect(db_conn_opt).await?;

    let blockscout_api = url::Url::from_str("https://blockscout.com/xdai/mainnet/api")
        .expect("Invalid blockscout api url");

    let client = Client::new(db, blockscout_api);

    let pending_addresses_handle = {
        let client = client.clone();
        tokio::spawn(async move { pending_addresses::extract(client).await })
    };

    let (pending_addresses_result,) = tokio::join!(pending_addresses_handle);
    pending_addresses_result??;

    Ok(())
}
