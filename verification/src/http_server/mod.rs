pub mod handlers;
mod routers;

pub use self::routers::{configure_router, AppRouter, Router};

use crate::config::Config;
use actix_web::{App, HttpServer};
use std::sync::Arc;

pub async fn run(config: Config) -> std::io::Result<()> {
    let socket_addr = config.server.addr;
    log::info!("Verification server is starting at {}", socket_addr);
    let app_config = Arc::new(
        AppRouter::new(config)
            .await
            .expect("couldn't initialize the app"),
    );
    HttpServer::new(move || {
        App::new().configure(|service_config| app_config.register_routes(service_config))
    })
    .bind(socket_addr)?
    .run()
    .await
}
