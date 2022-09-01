use anyhow::Context;
use multichain_api_gateway::{run, tracer::init_logs, Settings};
use std::error::Error;
use tracing::event;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::new().context("failed to parse config")?;
    event!(tracing::Level::INFO, settings = ?settings, "Got settings");
    init_logs(settings.jaeger.clone());
    run(settings)?.await?;
    Ok(())
}
