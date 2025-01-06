use crate::settings::{FetcherSettings, S3FetcherSettings};
use cron::Schedule;
use s3::{creds::Credentials, Bucket, Region};
use smart_contract_verifier::{
    DetailedVersion, Fetcher, FileValidator, ListFetcher, S3Fetcher, Version,
};
use std::{path::PathBuf, str::FromStr, sync::Arc};

pub async fn initialize_fetcher<Ver: Version>(
    fetcher_settings: FetcherSettings,
    compilers_dir: PathBuf,
    schedule: Schedule,
    validator: Option<Arc<dyn FileValidator<Ver>>>,
) -> anyhow::Result<Arc<dyn Fetcher<Version = Ver>>>
where
    <Ver as FromStr>::Err: std::fmt::Display,
{
    let fetcher: Arc<dyn Fetcher<Version = Ver>> = match fetcher_settings {
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

/// Normalizes the requested compiler version by matching it against a list of known compiler versions.
/// The function takes a [`DetailedVersion`] from the request and attempts to find a corresponding version
/// from the known list, allowing for cases where the requested commit hash is either a prefix or a longer
/// version than the known one. If a matching version is found, it is returned; otherwise, a
/// [`Status::invalid_argument`] error is returned.
pub fn normalize_request_compiler_version(
    compilers: &[DetailedVersion],
    request_compiler_version: &DetailedVersion,
) -> Result<DetailedVersion, tonic::Status> {
    let corresponding_known_compiler_version = compilers.iter().find(|&version| {
        return version.version() == request_compiler_version.version()
            && version.date() == request_compiler_version.date()
            && (version
                .commit()
                .starts_with(request_compiler_version.commit())
                || request_compiler_version
                    .commit()
                    .starts_with(version.commit()));
    });
    if let Some(compiler_version) = corresponding_known_compiler_version {
        Ok(compiler_version.clone())
    } else {
        Err(tonic::Status::invalid_argument(format!(
            "Compiler version not found: {request_compiler_version}"
        )))
    }
}
