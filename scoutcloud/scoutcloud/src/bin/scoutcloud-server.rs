use blockscout_service_launcher::launcher::ConfigSettings;
use scoutcloud::server::{run, Settings};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    run(settings).await
}
