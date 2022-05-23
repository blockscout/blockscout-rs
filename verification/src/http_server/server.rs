use crate::cli::Args;

use super::routes::config;
use actix_web::{middleware::Logger, App, HttpServer};

pub async fn run_server(args: Args) -> std::io::Result<()> {
    println!(
        "Verification server is starting at {}:{}",
        args.address, args.port
    );
    HttpServer::new(move || {
        let logger = Logger::default();

        App::new()
            .wrap(logger)
            .configure(config)
    })
    .bind((args.address, args.port))?
    .run()
    .await
}
