use actix_web::web;

use super::Router;
use crate::{
    compiler::Compilers,
    config::SolidityConfiguration,
    http_server::handlers::{files_input, standard_json},
    solidity::compiler_fetcher::{CompilerFetcher, Releases},
};

pub struct SolidityRouter {
    cache: web::Data<Compilers<CompilerFetcher>>,
}

impl SolidityRouter {
    pub async fn new(config: SolidityConfiguration) -> anyhow::Result<Self> {
        let releases = Releases::fetch_from_url(&config.compilers_list_url)
            .await
            .map_err(anyhow::Error::msg)?;
        let fetcher = CompilerFetcher::new(releases, "compilers/".into()).await;
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
                .route("/", web::post().to(files_input::verify))
                .route("/standard_json/", web::post().to(standard_json::verify)),
        );
    }
}
