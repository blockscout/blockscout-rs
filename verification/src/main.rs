use anyhow::Context;
use std::error::Error;
use verification::{init_logs, run_http_server, Settings};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let settings = Settings::new().context("failed to parse config")?;
    init_logs(settings.jaeger.clone());
    run_http_server(settings).await?;

    Ok(())
}
