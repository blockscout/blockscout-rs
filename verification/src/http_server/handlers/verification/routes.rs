use super::solidity::flatten;
use super::sourcify;
use crate::{compiler::download_cache::DownloadCache, solidity::svm_fetcher::SvmFetcher};
use actix_web::web;

pub fn config(service_config: &mut web::ServiceConfig) {
    let cache = DownloadCache::<SvmFetcher>::default();
    service_config
        .app_data(web::Data::new(cache))
        .route("/flatten", web::get().to(flatten::verify))
        .route("/sourcify", web::get().to(sourcify::verify));
}
