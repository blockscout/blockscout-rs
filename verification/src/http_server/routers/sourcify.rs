use actix_web::web;

use super::Router;
use crate::{
    http_server::handlers::sourcify::{self, SourcifyApiClient},
    settings::SourcifySettings,
};

pub struct SourcifyRouter {
    api_client: web::Data<SourcifyApiClient>,
}

impl SourcifyRouter {
    pub fn new(config: SourcifySettings) -> Self {
        let api_client = SourcifyApiClient::new(
            config.api_url,
            config.request_timeout,
            config.verification_attempts,
        );
        Self {
            api_client: web::Data::new(api_client),
        }
    }
}

impl Router for SourcifyRouter {
    fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .app_data(self.api_client.clone())
            .route("/verify", web::post().to(sourcify::verify));
    }
}
