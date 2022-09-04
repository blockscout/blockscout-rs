// use super::{configure_router, vyper::VyperRouter, Router, SolidityRouter, SourcifyRouter};
use super::{configure_router, solidity::SolidityRouter, Router};
use crate::{handlers::status, settings::Settings};
// use crate::{http_server::handlers::status, settings::Settings};
use actix_web::web;

pub struct AppRouter {
    solidity: Option<SolidityRouter>,
    // vyper: Option<VyperRouter>,
    // sourcify: Option<SourcifyRouter>,
}

impl AppRouter {
    pub async fn new(settings: Settings) -> anyhow::Result<Self> {
        let solidity = match settings.solidity.enabled {
            false => None,
            true => Some(SolidityRouter::new(settings.solidity).await?),
        };
        // let vyper = match settings.vyper.enabled {
        //     false => None,
        //     true => Some(VyperRouter::new(settings.vyper).await?),
        // };
        // let sourcify = settings
        //     .sourcify
        //     .enabled
        //     .then(|| SourcifyRouter::new(settings.sourcify));
        Ok(Self {
            solidity,
            // vyper,
            // sourcify,
        })
    }
}

impl Router for AppRouter {
    fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .route("/health", web::get().to(status::status))
            .service(
                web::scope("/api/v1")
                    .service(web::scope("/solidity").configure(configure_router(&self.solidity))),
                // .service(web::scope("/vyper").configure(configure_router(&self.vyper)))
                // .service(web::scope("/sourcify").configure(configure_router(&self.sourcify))),
            );
    }
}
