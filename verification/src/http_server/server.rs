use crate::cli::Args;

use super::routes::routes;
use actix_web::{App, HttpServer};

pub async fn run_server(args: Args) -> std::io::Result<()> {
    println!("Verification server is starting at {}:{}", args.address, args.port);
    HttpServer::new(|| App::new().configure(routes))
        .bind((args.address, args.port))?
        .run()
        .await
}
