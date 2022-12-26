use crate::{
    metrics,
    proto::{
        solidity_verifier_server::SolidityVerifier, ListCompilerVersionsRequest,
        ListCompilerVersionsResponse, VerifyResponse, VerifySolidityMultiPartRequest,
        VerifySolidityStandardJsonRequest,
    },
    settings::{Extensions, FetcherSettings, S3FetcherSettings, SoliditySettings},
    types::{
        StandardJsonParseError, VerifyResponseWrapper, VerifySolidityMultiPartRequestWrapper,
        VerifySolidityStandardJsonRequestWrapper,
    },
};
use s3::{creds::Credentials, Bucket, Region};
use smart_contract_verifier::{
    solidity, Compilers, Fetcher, ListFetcher, S3Fetcher, SolcValidator, SolidityClient,
    SolidityCompiler, VerificationError,
};
use std::{str::FromStr, sync::Arc};
use tokio::sync::Semaphore;
use tonic::{Request, Response, Status};

pub struct SolidityVerifierService {
    client: Arc<SolidityClient>,
}

impl SolidityVerifierService {
    pub async fn new(
        settings: SoliditySettings,
        compilers_threads_semaphore: Arc<Semaphore>,
        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_variables)] extensions: Extensions,
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
            client: Arc::new(client),
        })
    }
}

#[async_trait::async_trait]
impl SolidityVerifier for SolidityVerifierService {
    async fn verify_multi_part(
        &self,
        request: Request<VerifySolidityMultiPartRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifySolidityMultiPartRequestWrapper = request.into_inner().into();
        let result = solidity::multi_part::verify(self.client.clone(), request.try_into()?).await;

        if let Ok(verification_success) = result {
            let response = VerifyResponseWrapper::ok(verification_success);
            metrics::count_verify_contract(
                "solidity",
                response.status().as_str_name(),
                "multi-part",
            );
            return Ok(Response::new(response.into_inner()));
        }

        let err = result.unwrap_err();
        match err {
            VerificationError::Compilation(_)
            | VerificationError::NoMatchingContracts
            | VerificationError::CompilerVersionMismatch(_) => {
                Ok(Response::new(VerifyResponseWrapper::err(err).into_inner()))
            }
            VerificationError::Initialization(_) | VerificationError::VersionNotFound(_) => {
                Err(Status::invalid_argument(err.to_string()))
            }
            VerificationError::Internal(_) => Err(Status::internal(err.to_string())),
        }
    }

    async fn verify_standard_json(
        &self,
        request: Request<VerifySolidityStandardJsonRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifySolidityStandardJsonRequestWrapper = request.into_inner().into();
        let verification_request = {
            let request: Result<_, StandardJsonParseError> = request.try_into();
            if let Err(err) = request {
                match err {
                    StandardJsonParseError::InvalidContent(_) => {
                        return Err(Status::invalid_argument(err.to_string()))
                    }
                    StandardJsonParseError::BadRequest(_) => {
                        return Ok(Response::new(VerifyResponseWrapper::err(err).into_inner()))
                    }
                }
            }
            request.unwrap()
        };
        let result =
            solidity::standard_json::verify(self.client.clone(), verification_request).await;

        if let Ok(verification_success) = result {
            let response = VerifyResponseWrapper::ok(verification_success);
            metrics::count_verify_contract(
                "solidity",
                response.status().as_str_name(),
                "multi-part",
            );
            return Ok(Response::new(response.into_inner()));
        }

        let err = result.unwrap_err();
        match err {
            VerificationError::Compilation(_)
            | VerificationError::NoMatchingContracts
            | VerificationError::CompilerVersionMismatch(_) => {
                Ok(Response::new(VerifyResponseWrapper::err(err).into_inner()))
            }
            VerificationError::Initialization(_) | VerificationError::VersionNotFound(_) => {
                Err(Status::invalid_argument(err.to_string()))
            }
            VerificationError::Internal(_) => Err(Status::internal(err.to_string())),
        }
    }

    async fn list_compiler_versions(
        &self,
        _request: Request<ListCompilerVersionsRequest>,
    ) -> Result<Response<ListCompilerVersionsResponse>, Status> {
        let compiler_versions = self.client.compilers().all_versions_sorted_str();
        Ok(Response::new(ListCompilerVersionsResponse {
            compiler_versions,
        }))
    }
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
