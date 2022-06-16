use actix_web::web;

use super::Router;
use crate::http_server::handlers::{flatten, standard_json};
use crate::{compiler::download_cache::DownloadCache, solidity::github_fetcher::GithubFetcher};

pub struct SolidityRouter {
    cache: web::Data<DownloadCache<GithubFetcher>>,
}

impl SolidityRouter {
    pub async fn new() -> anyhow::Result<Self> {
        let fetcher = GithubFetcher::new("blockscout", "solc-bin", "compilers/".into())
            .await
            .map_err(anyhow::Error::msg)?;
        Ok(Self {
            cache: web::Data::new(DownloadCache::new(fetcher)),
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
