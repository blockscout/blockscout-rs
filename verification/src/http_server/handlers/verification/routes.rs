use super::solidity::{flatten, sourcify};
use actix_web::web;

pub fn config(service_config: &mut web::ServiceConfig) {
    service_config
        .route("/flatten", web::get().to(flatten::verify))
        .route("/sourcify", web::get().to(sourcify::verify));
}
