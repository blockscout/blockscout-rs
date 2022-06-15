use super::solidity::flatten;
use super::solidity::standard_json;
use super::sourcify;
use crate::{compiler::download_cache::DownloadCache, solidity::github_fetcher::GithubFetcher};
use actix_web::web;

pub fn config(service_config: &mut web::ServiceConfig) {
    let fetcher = futures::executor::block_on(GithubFetcher::new(
        "blockscout",
        "solc-bin",
        "compilers/".into(),
    ));
    let cache = DownloadCache::new(fetcher);
    service_config
        .app_data(web::Data::new(cache))
        .route("/flatten", web::post().to(flatten::verify))
        .route("/standard_json", web::post().to(standard_json::verify))
        .route("/sourcify", web::post().to(sourcify::verify));
}
