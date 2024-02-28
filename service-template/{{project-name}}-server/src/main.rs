use blockscout_service_launcher::launcher::ConfigSettings;
use {{crate_name}}_server::{Settings, run};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().expect("failed to read config");
    run(settings).await
}
