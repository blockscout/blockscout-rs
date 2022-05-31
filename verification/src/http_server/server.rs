use crate::configuration::Configuration;

use super::routes;
use actix_web::{App, HttpServer};
use log::info;

pub async fn run_server(configuration: Configuration) -> std::io::Result<()> {
    let socket_addr = configuration.server.addr;
    info!("Verification server is starting at {}", socket_addr);
    HttpServer::new(move || {
        App::new().configure(|service_config| routes::config(service_config, configuration.clone()))
    })
    .bind(socket_addr)?
    .run()
    .await
}
