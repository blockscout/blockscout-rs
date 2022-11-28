use super::router::Router;
use crate::{
    handlers::{solidity_multi_part, solidity_standard_json, solidity_version_list},
    settings::{Extensions, FetcherSettings, S3FetcherSettings, SoliditySettings},
};
use actix_web::web;
use s3::{creds::Credentials, Bucket, Region};
use smart_contract_verifier::{
    Compilers, Fetcher, ListFetcher, S3Fetcher, SolcValidator, SolidityClient, SolidityCompiler,
};
use std::{str::FromStr, sync::Arc};
use tokio::sync::Semaphore;

pub struct SolidityRouter {
    client: web::Data<SolidityClient>,
}

fn new_region(region: Option<String>, endpoint: Option<String>) -> Option<Region> {
    let region = region.unwrap_or_default();
    if let Some(endpoint) = endpoint {
        return Some(Region::Custom { region, endpoint });
    }

    // try to match with AWS regions, fail otherwise
    let region = Region::from_str(&region).ok()?;
    match region {
        Region::Custom {
            region: _,
            endpoint: _,
        } => None,
        region => Some(region),
    }
}

fn new_bucket(settings: &S3FetcherSettings) -> anyhow::Result<Arc<Bucket>> {
    let region = new_region(settings.region.clone(), settings.endpoint.clone())
        .ok_or_else(|| anyhow::anyhow!("got invalid region/endpoint settings"))?;
    let bucket = Arc::new(Bucket::new(
        &settings.bucket,
        region,
        Credentials::new(
            settings.access_key.as_deref(),
            settings.secret_key.as_deref(),
            None,
            None,
            None,
        )?,
    )?);
    Ok(bucket)
}

impl SolidityRouter {
    pub async fn new(
        settings: SoliditySettings,
        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_variables)] extensions: Extensions,
        compilers_threads_semaphore: Arc<Semaphore>,
    ) -> anyhow::Result<Self> {
        let dir = settings.compilers_dir.clone();
        let schedule = settings.refresh_versions_schedule;
        let validator = Arc::new(SolcValidator::default());
        let fetcher: Arc<dyn Fetcher> = match settings.fetcher {
            FetcherSettings::List(list_settings) => Arc::new(
                ListFetcher::new(
                    list_settings.list_url,
                    settings.compilers_dir,
                    Some(schedule),
                    Some(validator),
                )
                .await?,
            ),
            FetcherSettings::S3(s3_settings) => Arc::new(
                S3Fetcher::new(
                    new_bucket(&s3_settings)?,
                    settings.compilers_dir,
                    Some(schedule),
                    Some(validator),
                )
                .await?,
            ),
        };
        let compilers = Compilers::new(
            fetcher,
            SolidityCompiler::new(),
            compilers_threads_semaphore,
        );
        compilers.load_from_dir(&dir).await;

        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_mut)]
        let mut client = SolidityClient::new(compilers);

        #[cfg(feature = "sig-provider-extension")]
        if let Some(sig_provider) = extensions.sig_provider {
            // TODO(#221): create only one instance of middleware/connection
            client = client
                .with_middleware(sig_provider_extension::SigProvider::new(sig_provider).await?);
        }

        Ok(Self {
            client: web::Data::new(client),
        })
    }
}

impl Router for SolidityRouter {
    fn register_routes(&self, service_config: &mut web::ServiceConfig) {
        service_config
            .app_data(self.client.clone())
            .service(
                web::scope("/verify")
                    .route(
                        "/multiple-files",
                        web::post().to(solidity_multi_part::verify),
                    )
                    .route(
                        "/standard-json",
                        web::post().to(solidity_standard_json::verify),
                    ),
            )
            .route(
                "/versions",
                web::get().to(solidity_version_list::get_version_list),
            );
    }
}
