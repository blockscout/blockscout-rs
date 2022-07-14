use super::Router;
use crate::{
    compiler::{Compilers, ListFetcher},
    config::SolidityConfiguration,
    http_server::handlers::{multi_part, standard_json, version_list},
};
use actix_web::web;
use std::{path::PathBuf, str::FromStr, sync::Arc};

pub struct SolidityRouter {
    compilers: web::Data<Compilers>,
}

impl SolidityRouter {
    pub async fn new(config: SolidityConfiguration) -> anyhow::Result<Self> {
        let fetcher = Arc::new(
            ListFetcher::new(
                config.compilers_list_url,
                Some(config.refresh_versions_schedule),
                "compilers/".into(),
            )
            .await?,
        );
        let compilers = Compilers::new(fetcher);
        compilers.load_from_dir().await;
        Ok(Self {
            compilers: web::Data::new(compilers),
        })
    }
}

impl Router for SolidityRouter {
    fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .app_data(self.compilers.clone())
            .service(
                web::scope("/verify")
                    .route("/multiple-files", web::post().to(multi_part::verify))
                    .route("/standard-json", web::post().to(standard_json::verify)),
            )
            .route("/versions", web::get().to(version_list::get_version_list));
    }
}
