use super::solidity::flatten;
use super::solidity::standard_json;
use super::sourcify;
use crate::{compiler::download_cache::DownloadCache, solidity::github_fetcher::GithubFetcher};
use actix_web::web;

pub struct AppConfig {
    cache: web::Data<DownloadCache<GithubFetcher>>,
}

impl AppConfig {
    pub async fn new() -> anyhow::Result<Self> {
        let fetcher = GithubFetcher::new("blockscout", "solc-bin", "compilers/".into())
            .await
            .map_err(anyhow::Error::msg)?;
        Ok(Self {
            cache: web::Data::new(DownloadCache::new(fetcher)),
        })
    }

    pub fn config(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .app_data(self.cache.clone())
            .route("/flatten", web::get().to(flatten::verify))
            .route("/standard_json", web::get().to(standard_json::verify))
            .route("/sourcify", web::get().to(sourcify::verify));
    }
}
