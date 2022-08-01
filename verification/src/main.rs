use verification::{run_http_server, Settings};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let config = Settings::new();
    run_http_server(config).await
}
