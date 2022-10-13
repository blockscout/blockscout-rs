use super::router::Router;
use crate::{
    handlers::{vyper_multi_part, vyper_version_list},
    settings::{FetcherSettings, VyperSettings},
};
use actix_web::web;
use smart_contract_verifier::{
    Compilers, ListFetcher, VyperClient, VyperClientBuilder, VyperCompiler,
};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct VyperRouter {
    client: web::Data<VyperClient>,
}

impl VyperRouter {
    pub async fn new(
        settings: VyperSettings,
        compilers_threads_semaphore: Arc<Semaphore>,
    ) -> anyhow::Result<Self> {
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
        let compilers = Compilers::new(fetcher, VyperCompiler::new(), compilers_threads_semaphore);
        compilers.load_from_dir(&dir).await;
        let client = VyperClientBuilder::new(compilers).build();
        Ok(Self {
            client: web::Data::new(client),
        })
    }
}

impl Router for VyperRouter {
    fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .app_data(self.client.clone())
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
