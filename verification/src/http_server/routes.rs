use actix_web::web;

use crate::Config;

use super::handlers::{status::status, verification};

pub fn config(service_config: &mut web::ServiceConfig, app_config: Config) {
    service_config
        .app_data(web::Data::new(app_config))
        .route("/health", web::get().to(status))
        .service(
            web::scope("/api/v1")
                .service(web::scope("/verification").configure(verification::routes::config)),
        );
}
