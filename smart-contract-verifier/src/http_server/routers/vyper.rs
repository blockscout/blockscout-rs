use std::sync::Arc;

use actix_web::web;

use crate::{
    compiler::{Compilers, ListFetcher},
    http_server::handlers::vyper,
    settings::VyperSettings,
    vyper::VyperCompilerAgent,
    Router,
};

pub struct VyperRouter {
    compilers: web::Data<Compilers<VyperCompilerAgent>>,
}

impl VyperRouter {
    pub async fn new(settings: VyperSettings) -> anyhow::Result<Self> {
        let dir = settings.compilers_dir.clone();
        let fetcher = Arc::new(
            ListFetcher::new(
                settings.list_url,
                settings.compilers_dir,
                Some(settings.refresh_versions_schedule),
            )
            .await?,
        );
        let compilers = Compilers::new(fetcher, VyperCompilerAgent::new());
        compilers.load_from_dir(&dir).await;
        Ok(Self {
            compilers: web::Data::new(compilers),
        })
    }
}

impl Router for VyperRouter {
    fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .app_data(self.compilers.clone())
            .service(
                web::scope("/verify")
                    .route("/multiple-files", web::post().to(vyper::multi_part::verify)),
            )
            .route(
                "/versions",
                web::get().to(vyper::version_list::get_version_list),
            );
    }
}
