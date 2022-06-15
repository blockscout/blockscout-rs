use actix_web::web;

use super::Router;
use crate::{
    compiler::download_cache::DownloadCache,
    http_server::handlers::{flatten, standard_json},
    solidity::github_fetcher::GithubFetcher,
};

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
        service_config
            .app_data(self.cache.clone())
            .route("/flatten", web::get().to(flatten::verify))
            .route("/standard_json", web::get().to(standard_json::verify));
    }
}
