use crate::configuration::Configuration;

use super::routes;
use actix_web::{web, App, HttpServer};
use log::info;

pub async fn run_server(config: Configuration) -> std::io::Result<()> {
    let socket_addr = config.server.addr;
    info!("Verification server is starting at {}", socket_addr);
    HttpServer::new(move || {
        App::new()
            .configure(routes::config)
            .app_data(web::Data::new(config.clone()))
    })
    .bind(socket_addr)?
    .run()
    .await
}
