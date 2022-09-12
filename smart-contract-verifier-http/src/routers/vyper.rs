use super::router::Router;
use crate::{
    handlers::{vyper_multi_part, vyper_version_list},
    settings::{FetcherSettings, VyperSettings},
};
use actix_web::web;
use smart_contract_verifier::{Compilers, ListFetcher, VyperCompiler};
use std::sync::Arc;

pub struct VyperRouter {
    compilers: web::Data<Compilers<VyperCompiler>>,
}

impl VyperRouter {
    pub async fn new(settings: VyperSettings) -> anyhow::Result<Self> {
        let dir = settings.compilers_dir.clone();
        let list_url = match settings.fetcher {
            FetcherSettings::List(s) => s.list_url,
            FetcherSettings::S3(_) => {
                return Err(anyhow::anyhow!("S3 fetcher for vyper not supported"))
            }
        };
        let fetcher = Arc::new(
            ListFetcher::new(
                list_url,
                settings.compilers_dir,
                Some(settings.refresh_versions_schedule),
                None,
            )
            .await?,
        );
        let compilers = Compilers::new(fetcher, VyperCompiler::new());
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
                    .route("/multiple-files", web::post().to(vyper_multi_part::verify)),
            )
            .route(
                "/versions",
                web::get().to(vyper_version_list::get_version_list),
            );
    }
}
