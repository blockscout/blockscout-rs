mod http_server;
mod solidity;
mod cli;

use http_server::server::run_server;
use clap::Parser;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = cli::Args::parse();
    run_server(args).await
}
