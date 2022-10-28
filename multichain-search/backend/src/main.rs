use anyhow::Context;
use multichain_search::{init_logs, server::run, Settings};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::new().context("failed to parse config")?;
    init_logs(settings.jaeger.clone());
    tracing::info!(instances = ?settings.blockscout.instances, addr = ?settings.server.addr, "Start server");
    run(settings)?.await?;
    Ok(())
}
