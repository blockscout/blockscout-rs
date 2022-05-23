mod cli;
mod config;
mod http_server;
mod solidity;

use config::Config;
use http_server::server::run_server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let config = Config::parse();
    run_server(config).await
}
