use crate::{config::Config};

use super::routes;
use actix_web::{middleware::Logger, App, HttpServer};

pub async fn run_server(config: Config) -> std::io::Result<()> {
    println!("Verification server is starting at {}", config.socket_addr);
    HttpServer::new(move || {
        let logger = Logger::default();

        App::new().wrap(logger).configure(routes::config)
    })
    .bind(config.socket_addr)?
    .run()
    .await
}
