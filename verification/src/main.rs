mod http_server;
use http_server::server::run_server;
use verification::{parse_args, CLIArgs};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = parse_args();
    run_server(args).await
}
