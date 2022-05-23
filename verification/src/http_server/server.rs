use crate::config::Config;

use super::routes;
use actix_web::{App, HttpServer};
use log::info;

pub async fn run_server(config: Config) -> std::io::Result<()> {
    info!("Verification server is starting at {}", config.socket_addr);
    HttpServer::new(move || App::new().configure(routes::config))
        .bind(config.socket_addr)?
        .run()
        .await
}
