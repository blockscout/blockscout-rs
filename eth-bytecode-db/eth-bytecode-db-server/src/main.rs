use blockscout_service_launcher::launcher::ConfigSettings;
use eth_bytecode_db_server::Settings;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    eth_bytecode_db_server::run(settings).await
}
