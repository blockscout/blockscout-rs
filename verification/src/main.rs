mod http_server;
mod solidity;
mod cli;

use http_server::server::run_server;


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = cli::parse_args();
    run_server(args).await
}
