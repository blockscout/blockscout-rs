use super::solidity::flatten;
use super::solidity::standard_json;
use super::sourcify;
use crate::{download_cache::DownloadCache, solidity::fetcher::SvmFetcher};
use actix_web::web;

pub fn config(service_config: &mut web::ServiceConfig) {
    let cache = DownloadCache::<SvmFetcher>::default();
    service_config
        .app_data(web::Data::new(cache))
        .route("/flatten", web::get().to(flatten::verify))
        .route("/standard_json", web::get().to(standard_json::verify))
        .route("/sourcify", web::get().to(sourcify::verify));
}
