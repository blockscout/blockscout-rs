use super::{configure_router, Router, SolidityRouter, SourcifyRouter};
use crate::{config::Config, http_server::handlers::status};
use actix_web::web;

pub struct AppRouter {
    solidity: Option<SolidityRouter>,
    sourcify: Option<SourcifyRouter>,
}

impl AppRouter {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let solidity = match config.verifier.enabled {
            false => None,
            true => Some(SolidityRouter::new().await?),
        };
        let sourcify = config
            .sourcify
            .enabled
            .then(|| SourcifyRouter::new(config.sourcify));
        Ok(Self { solidity, sourcify })
    }
}

impl Router for AppRouter {
    fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .route("/health", web::get().to(status::status))
            .service(
                web::scope("/api/v1")
                    .service(web::scope("/solidity").configure(configure_router(&self.solidity)))
                    .service(web::scope("/sourcify").configure(configure_router(&self.sourcify))),
            );
    }
}
