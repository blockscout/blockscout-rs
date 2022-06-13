use actix_web::web;

use crate::Config;

use super::handlers::{status::status, verification};

pub struct AppConfig {
    config: web::Data<Config>,
    verification: verification::routes::AppConfig,
}

impl AppConfig {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        Ok(Self {
            config: web::Data::new(config),
            verification: verification::routes::AppConfig::new().await?,
        })
    }

    pub fn config(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .app_data(self.config.clone())
            .route("/health", web::get().to(status))
            .service(
                web::scope("/api/v1").service(
                    web::scope("/verification")
                        .configure(|service_config| self.verification.config(service_config)),
                ),
            );
    }
}
