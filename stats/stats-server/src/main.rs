use blockscout_service_launcher::launcher::ConfigSettings;
use stats_server::{Settings, stats};
use tracing::log;

fn log_error(err: anyhow::Error) -> anyhow::Error {
    log::error!("service failed with error: {}", err);
    err
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::build().map_err(log_error)?;
    stats(settings, None).await.map_err(log_error)
}
