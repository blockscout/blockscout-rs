use actix_web::web;

use crate::http_server::handlers::{
    health::get_health,
    verifiers::{source_code, sourcify, standard_json, viper},
};

pub fn routes(service_config: &mut web::ServiceConfig) {
    service_config
        .route("/health", web::get().to(get_health))
        .service(
            web::scope("/api/v1").service(
                web::scope("verification")
                    .route("/flatten/", web::post().to(source_code::verificate))
                    .route("/sourcify/", web::post().to(sourcify::verificate))
                    .route("/viper/", web::post().to(viper::verificate))
                    .route("/standard_json/", web::post().to(standard_json::verificate)),
            ),
        );
}
