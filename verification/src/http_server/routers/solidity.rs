use super::Router;
use crate::{
    compiler::{Compilers, Fetcher, ListFetcher, S3Fetcher},
    http_server::handlers::{multi_part, standard_json, version_list},
    settings::{FetcherSettings, S3FetcherSettings, SoliditySettings},
};
use actix_web::web;
use s3::{creds::Credentials, Bucket, Region};
use std::{str::FromStr, sync::Arc};

pub struct SolidityRouter {
    compilers: web::Data<Compilers>,
}

fn new_region(region: Option<String>, endpoint: Option<String>) -> Option<Region> {
    let region = region.unwrap_or_else(|| "".to_string());
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

fn new_bucket(config: &S3FetcherSettings) -> anyhow::Result<Arc<Bucket>> {
    let region = new_region(config.region.clone(), config.endpoint.clone())
        .ok_or_else(|| anyhow::anyhow!("got invalid region/endpoint config"))?;
    let bucket = Arc::new(Bucket::new(
        &config.bucket,
        region,
        Credentials::new(
            config.access_key.as_deref(),
            config.secret_key.as_deref(),
            None,
            None,
            None,
        )?,
    )?);
    Ok(bucket)
}

impl SolidityRouter {
    pub async fn new(config: SoliditySettings) -> anyhow::Result<Self> {
        let dir = config.compiler_folder.clone();
        let fetcher: Arc<dyn Fetcher> = match config.fetcher {
            FetcherSettings::List(fetcher_config) => Arc::new(
                ListFetcher::new(
                    fetcher_config.compilers_list_url,
                    config.compiler_folder,
                    Some(fetcher_config.refresh_versions_schedule),
                )
                .await?,
            ),
            FetcherSettings::S3(s3_config) => Arc::new(
                S3Fetcher::new(
                    new_bucket(&s3_config)?,
                    config.compiler_folder,
                    Some(s3_config.refresh_versions_schedule),
                )
                .await?,
            ),
        };
        let compilers = Compilers::new(fetcher);
        compilers.load_from_dir(&dir).await;
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
