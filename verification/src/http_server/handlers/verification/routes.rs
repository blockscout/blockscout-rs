use super::solidity::flatten;
use crate::{download_cache::DownloadCache, solidity::fetcher::SvmFetcher};
use actix_web::web;

pub fn config(service_config: &mut web::ServiceConfig) {
    let cache = DownloadCache::<SvmFetcher>::default();
    service_config
        .app_data(web::Data::new(cache))
        .route("/flatten", web::get().to(flatten::verify));
}
