use crate::settings::{FetcherSettings, S3FetcherSettings};
use cron::Schedule;
use s3::{creds::Credentials, Bucket, Region};
use smart_contract_verifier::{Fetcher, FileValidator, ListFetcher, S3Fetcher, SolcValidator};
use std::{path::PathBuf, str::FromStr, sync::Arc};

pub async fn initialize_fetcher(
    fetcher_settings: FetcherSettings,
    compilers_dir: PathBuf,
    schedule: Schedule,
    validator: Option<Arc<dyn FileValidator>>,
) -> anyhow::Result<Arc<dyn Fetcher>> {
    let fetcher: Arc<dyn Fetcher> = match fetcher_settings {
        FetcherSettings::List(list_settings) => Arc::new(
            ListFetcher::new(
                list_settings.list_url,
                compilers_dir,
                Some(schedule),
                validator,
            )
            .await?,
        ),
        FetcherSettings::S3(s3_settings) => Arc::new(
            S3Fetcher::new(
                new_bucket(&s3_settings)?,
                compilers_dir,
                Some(schedule),
                validator,
            )
            .await?,
        ),
    };

    Ok(fetcher)
}

pub async fn initialize_generic_fetcher<Ver: smart_contract_verifier::generic_fetcher::Version>(
    fetcher_settings: FetcherSettings,
    compilers_dir: PathBuf,
    schedule: Schedule,
    validator: Option<Arc<dyn smart_contract_verifier::generic_fetcher::FileValidator<Ver>>>,
) -> anyhow::Result<Arc<dyn smart_contract_verifier::generic_fetcher::Fetcher<Version = Ver>>>
where
    <Ver as FromStr>::Err: std::fmt::Display,
{
    let fetcher: Arc<dyn smart_contract_verifier::generic_fetcher::Fetcher<Version = Ver>> =
        match fetcher_settings {
            FetcherSettings::List(list_settings) => Arc::new(
                smart_contract_verifier::generic_list_fetcher::ListFetcher::new(
                    list_settings.list_url,
                    compilers_dir,
                    Some(schedule),
                    validator,
                )
                .await?,
            ),
            FetcherSettings::S3(s3_settings) => Arc::new(
                smart_contract_verifier::generic_s3_fetcher::S3Fetcher::new(
                    new_bucket(&s3_settings)?,
                    compilers_dir,
                    Some(schedule),
                    validator,
                )
                .await?,
            ),
        };

    Ok(fetcher)
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
