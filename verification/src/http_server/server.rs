use crate::{config::Config, routes::AppConfig};
use actix_web::{App, HttpServer};
use log::info;
use std::sync::Arc;

pub async fn run_server(config: Config) -> std::io::Result<()> {
    let socket_addr = config.server.addr;
    info!("Verification server is starting at {}", socket_addr);
    let app_config = Arc::new(
        AppConfig::new(config)
            .await
            .expect("couldn't initialize the app"),
    );
    HttpServer::new(move || {
        App::new().configure(|service_config| app_config.config(service_config))
    })
    .bind(socket_addr)?
    .run()
    .await
}
