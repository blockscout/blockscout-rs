use actix_web::web;

use super::SourcifyApiClient;
use crate::config::SourcifyConfiguration;

pub struct SourcifyClient {
    api_client: web::Data<SourcifyApiClient>,
}

impl SourcifyClient {
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

    pub fn config(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .app_data(self.api_client.clone())
            .route("/sourcify", web::get().to(super::verify));
    }
}
