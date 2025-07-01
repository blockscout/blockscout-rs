use blockscout_service_launcher::launcher::ConfigSettings;
use da_indexer_server::Settings;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    da_indexer_server::run(settings).await
}
