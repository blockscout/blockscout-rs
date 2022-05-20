use crate::cli::CLIArgs;

use super::routes::routes;
use actix_web::{App, HttpServer};

pub async fn run_server(args: CLIArgs) -> std::io::Result<()> {
    println!("🚀 starting server at {}:{}", args.address, args.port);
    HttpServer::new(|| App::new().configure(routes))
        .bind((args.address, args.port))?
        .run()
        .await
}
