use super::solidity::flatten;
use super::sourcify;
use crate::{compiler::download_cache::DownloadCache, solidity::github_fetcher::GithubFetcher};
use actix_web::web;

pub fn config(service_config: &mut web::ServiceConfig) {
    let fetcher = tokio::task::block_in_place(move || {
        futures::executor::block_on(GithubFetcher::new(
            "blockscout",
            "solc-bin",
            "compilers/".into(),
        ))
    })
    .expect("couldn't initialize github fetcher");
    let cache = DownloadCache::new(fetcher);
    service_config
        .app_data(web::Data::new(cache))
        .route("/flatten", web::get().to(flatten::verify))
        .route("/sourcify", web::get().to(sourcify::verify));
}
