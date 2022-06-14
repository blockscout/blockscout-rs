use actix_web::web;

use crate::http_server::handlers::verification::SourcifyClient;
use crate::Config;

use super::handlers::{status, verification::VerificationClient};

pub struct AppConfig {
    verification: Option<VerificationClient>,
    sourcify: Option<SourcifyClient>,
}

impl AppConfig {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let verification = match config.verifier.disabled {
            true => None,
            false => Some(VerificationClient::new().await?),
        };
        let sourcify = match config.sourcify.disabled {
            true => None,
            false => Some(SourcifyClient::new(config.sourcify)),
        };
        Ok(Self {
            verification,
            sourcify,
        })
    }

    pub fn config(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .route("/health", web::get().to(status::status))
            .service(
                web::scope("/api/v1").service(
                    web::scope("/verification")
                        .configure(|service_config| {
                            if let Some(client) = self.verification.as_ref() {
                                client.config(service_config)
                            }
                        })
                        .configure(|service_config| {
                            if let Some(client) = self.sourcify.as_ref() {
                                client.config(service_config)
                            }
                        }),
                ),
            );
    }
}
