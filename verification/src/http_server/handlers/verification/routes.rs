use super::solidity::flatten;
use crate::solidity::compiler_cache::CompilerCache;
use actix_web::web;

pub fn config(service_config: &mut web::ServiceConfig) {
    let cache = CompilerCache::default();
    service_config
        .app_data(web::Data::new(cache))
        .route("/flatten", web::get().to(flatten::verify));
}
