use basic_cache_server::{run, Settings};
use blockscout_service_launcher::launcher::ConfigSettings;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    run(settings).await
}
