use anyhow::Context;
use multichain_api_gateway::{run, Settings};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::new().context("failed to parse config")?;
    run(settings)?.await?;
    Ok(())
}
