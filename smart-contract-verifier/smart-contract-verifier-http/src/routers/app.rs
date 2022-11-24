use super::{
    router::{configure_router, Router},
    solidity::SolidityRouter,
    sourcify::SourcifyRouter,
    vyper::VyperRouter,
};
use crate::{handlers::status, settings::Settings};
use actix_web::web;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct AppRouter {
    solidity: Option<SolidityRouter>,
    vyper: Option<VyperRouter>,
    sourcify: Option<SourcifyRouter>,
}

impl AppRouter {
    pub async fn new(settings: Settings) -> anyhow::Result<Self> {
        let compilers_lock = Arc::new(Semaphore::new(settings.compilers.max_threads.get()));
        let solidity = match settings.solidity.enabled {
            false => None,
            true => Some(
                SolidityRouter::new(
                    settings.solidity,
                    settings.extensions.solidity,
                    compilers_lock.clone(),
                )
                .await?,
            ),
        };
        let vyper = match settings.vyper.enabled {
            false => None,
            true => Some(
                VyperRouter::new(settings.vyper, settings.extensions.vyper, compilers_lock).await?,
            ),
        };
        let sourcify = match settings.sourcify.enabled {
            false => None,
            true => {
                Some(SourcifyRouter::new(settings.sourcify, settings.extensions.sourcify).await?)
            }
        };
        Ok(Self {
            solidity,
            vyper,
            sourcify,
        })
    }
}

impl Router for AppRouter {
    fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .route("/health", web::get().to(status::status))
            .service(
                web::scope("/api/v1")
                    .service(web::scope("/solidity").configure(configure_router(&self.solidity)))
                    .service(web::scope("/vyper").configure(configure_router(&self.vyper)))
                    .service(web::scope("/sourcify").configure(configure_router(&self.sourcify))),
            );
    }
}
