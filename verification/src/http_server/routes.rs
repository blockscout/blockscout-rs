use crate::Config;
use actix_web::web;

use super::handlers::{status::status, verification};

use super::handlers::verification::SourcifyApiClient;

pub fn config(service_config: &mut web::ServiceConfig, app_config: Config) {
    let sourcify_client = SourcifyApiClient::new(app_config.sourcify.api_url.clone());
    service_config
        .app_data(web::Data::new(app_config))
        .app_data(web::Data::new(sourcify_client))
        .route("/health", web::get().to(status))
        .service(
            web::scope("/api/v1")
                .service(web::scope("/verification").configure(verification::routes::config)),
        );
}
