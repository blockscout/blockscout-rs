use actix_web::web;

use super::solidity::flatten;

pub fn config(service_config: &mut web::ServiceConfig) {
    service_config.route("/flatten", web::get().to(flatten::verify));
}
