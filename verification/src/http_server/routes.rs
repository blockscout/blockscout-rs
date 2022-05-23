use actix_web::web;

use super::handlers::{status::status, verification};

pub fn config(service_config: &mut web::ServiceConfig) {
    service_config
        .route("/health", web::get().to(status))
        .service(
            web::scope("/api/v1")
                .service(web::scope("/verification").configure(verification::routes::config)),
        );
}
