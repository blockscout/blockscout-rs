use stats_server::{stats, Settings};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::new().expect("failed to read config");
    stats(settings).await
}
