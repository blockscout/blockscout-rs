use crate::config::Config;

use super::routes;
use actix_web::{App, HttpServer};
use log::info;

pub async fn run_server(config: Config) -> std::io::Result<()> {
    let socket_addr = config.server.addr;
    info!("Verification server is starting at {}", socket_addr);
    HttpServer::new(move || {
        App::new().configure(|service_config| routes::config(service_config, config.clone()))
    })
    .bind(socket_addr)?
    .run()
    .await
}
