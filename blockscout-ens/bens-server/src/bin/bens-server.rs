use bens_server::Settings;
use blockscout_service_launcher::launcher::ConfigSettings;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    bens_server::run(settings).await
}
