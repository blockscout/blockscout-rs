use blockscout_service_launcher::launcher::ConfigSettings;
use proxy_verifier_server::Settings;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    proxy_verifier_server::run(settings).await
}
