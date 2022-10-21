use super::router::Router;
use crate::{handlers::sourcify, settings::SourcifySettings};
use actix_web::web;
use smart_contract_verifier::SourcifyApiClient;

pub struct SourcifyRouter {
    api_client: web::Data<SourcifyApiClient>,
}

impl SourcifyRouter {
    pub fn new(settings: SourcifySettings) -> Self {
        let api_client = SourcifyApiClient::new(
            settings.api_url,
            settings.request_timeout,
            settings.verification_attempts,
        )
        .expect("failed to build sourcify client");
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
