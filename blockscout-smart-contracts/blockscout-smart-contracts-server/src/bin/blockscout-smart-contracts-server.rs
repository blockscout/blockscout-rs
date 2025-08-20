use blockscout_service_launcher::launcher::ConfigSettings;
use blockscout_smart_contracts_server::{Settings, run};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    run(settings).await
}
