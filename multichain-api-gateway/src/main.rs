use anyhow::Context;
use multichain_api_gateway::{run, Settings};
use std::error::Error;
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt, Layer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let stdout = tracing_subscriber::fmt::layer().with_filter(
        tracing_subscriber::EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env_lossy(),
    );
    let registry = tracing_subscriber::registry()
        // output logs (tracing) to stdout with log level taken from env (default is INFO)
        .with(stdout);
    registry
        .try_init()
        .expect("failed to register tracer with registry");

    let settings = Settings::new().context("failed to parse config")?;
    run(settings)?.await?;
    Ok(())
}
