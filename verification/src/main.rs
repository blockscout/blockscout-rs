use verification::run_http_server;
use verification::Config;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let config = Config::parse();
    run_http_server(config).await
}
