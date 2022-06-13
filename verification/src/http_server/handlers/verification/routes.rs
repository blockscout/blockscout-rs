use super::solidity::flatten;
use super::solidity::standard_json;
use super::sourcify;
use crate::{compiler::download_cache::DownloadCache, solidity::github_fetcher::GithubFetcher};
use actix_web::web;

pub fn config(service_config: &mut web::ServiceConfig) {
    let fetcher = tokio::task::block_in_place(move || {
        futures::executor::block_on(GithubFetcher::new(
            "blockscout-rs",
            "solc-bin",
            "compilers/".into(),
        ))
    })
    .expect("couldn't initialize github fetcher");
    let cache = DownloadCache::new(fetcher);
    service_config
        .app_data(web::Data::new(cache))
        .route("/flatten", web::get().to(flatten::verify))
        .route("/standard_json", web::get().to(standard_json::verify))
        .route("/sourcify", web::get().to(sourcify::verify));
}
