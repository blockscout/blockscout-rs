use anyhow::Context;
use env_logger::Env;
use multichain_api_gateway::{run, Settings};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let settings = Settings::new().context("failed to parse config")?;
    run(settings)?.await?;
    Ok(())
}
