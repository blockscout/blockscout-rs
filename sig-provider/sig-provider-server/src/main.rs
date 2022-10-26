use sig_provider_server::{sig_provider, Settings};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let settings = Settings::new().expect("failed to read config");
    sig_provider(settings).await
}
