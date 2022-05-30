use verification::run_http_server;
use verification::Configuration;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let config = Configuration::parse().expect("Failed to parse config");
    run_http_server(config).await
}
