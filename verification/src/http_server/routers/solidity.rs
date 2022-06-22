use actix_web::web;

use super::Router;
use crate::{
    compiler::Compilers,
    config::CompilerConfiguration,
    http_server::handlers::{flatten, standard_json},
    solidity::compiler_fetcher::CompilerFetcher,
};

pub struct SolidityRouter {
    cache: web::Data<Compilers<CompilerFetcher>>,
}

impl SolidityRouter {
    pub async fn new(config: CompilerConfiguration) -> anyhow::Result<Self> {
        let fetcher = CompilerFetcher::new(&config.compilers_list_url, "compilers/".into())
            .await
            .map_err(anyhow::Error::msg)?;
        let compilers = Compilers::new(fetcher);
        Ok(Self {
            cache: web::Data::new(compilers),
        })
    }
}

impl Router for SolidityRouter {
    fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config.app_data(self.cache.clone()).service(
            web::scope("/verify")
                .route("/flatten", web::post().to(flatten::verify))
                .route("/standard_json", web::post().to(standard_json::verify)),
        );
    }
}
