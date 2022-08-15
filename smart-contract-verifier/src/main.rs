use anyhow::Context;
use std::error::Error;
use smart_contract_verifier::{init_logs, run_http_server, Settings};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::new().context("failed to parse config")?;
    init_logs(settings.jaeger.clone());
    run_http_server(settings).await?;

    Ok(())
}
