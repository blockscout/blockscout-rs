use super::{SolidityRouter, SourcifyRouter};
use crate::{config::Config, http_server::handlers::status};
use actix_web::web;

pub struct AppRouter {
    verification: Option<SolidityRouter>,
    sourcify: Option<SourcifyRouter>,
}

impl AppRouter {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let verification = match config.verifier.disabled {
            true => None,
            false => Some(SolidityRouter::new().await?),
        };
        let sourcify = match config.sourcify.disabled {
            true => None,
            false => Some(SourcifyRouter::new(config.sourcify)),
        };
        Ok(Self {
            verification,
            sourcify,
        })
    }

    pub fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .route("/health", web::get().to(status::status))
            .service(
                web::scope("/api/v1").service(
                    web::scope("/verification")
                        .configure(|service_config| {
                            if let Some(client) = self.verification.as_ref() {
                                client.register_routes(service_config)
                            }
                        })
                        .configure(|service_config| {
                            if let Some(client) = self.sourcify.as_ref() {
                                client.register_routes(service_config)
                            }
                        }),
                ),
            );
    }
}
