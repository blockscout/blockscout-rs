use verification::{run_http_server, Config};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let config = Config::parse().expect("Failed to parse config");
    run_http_server(config).await
}
