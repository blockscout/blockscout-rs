use actix_web::web;

use crate::config::SourcifyConfiguration;
use crate::http_server::handlers::sourcify::{self, SourcifyApiClient};

pub struct SourcifyRouter {
    api_client: web::Data<SourcifyApiClient>,
}

impl SourcifyRouter {
    pub fn new(config: SourcifyConfiguration) -> Self {
        let api_client = SourcifyApiClient::new(
            config.api_url,
            config.request_timeout,
            config.verification_attempts,
        );
        Self {
            api_client: web::Data::new(api_client),
        }
    }

    pub fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .app_data(self.api_client.clone())
            .route("/sourcify", web::get().to(sourcify::verify));
    }
}
