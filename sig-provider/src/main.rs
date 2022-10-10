use sig_provider::{sig_provider, Settings};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::new().expect("failed to read config");
    sig_provider(settings).await
}
