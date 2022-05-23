mod cli;
mod http_server;
mod solidity;

use clap::Parser;
use http_server::server::run_server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = cli::Args::parse();
    env_logger::init();
    run_server(args).await
}
