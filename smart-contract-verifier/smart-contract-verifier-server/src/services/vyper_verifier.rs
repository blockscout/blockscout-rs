use crate::{
    metrics,
    proto::{
        vyper_verifier_server::VyperVerifier, ListCompilerVersionsRequest,
        ListCompilerVersionsResponse, VerifyResponse, VerifyVyperMultiPartRequest,
    },
    settings::{Extensions, FetcherSettings, VyperSettings},
    types::{VerifyResponseWrapper, VerifyVyperMultiPartRequestWrapper},
};
use smart_contract_verifier::{
    vyper, Compilers, ListFetcher, VerificationError, VyperClient, VyperCompiler,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tonic::{Request, Response, Status};

pub struct VyperVerifierService {
    client: Arc<VyperClient>,
}

impl VyperVerifierService {
    pub async fn new(
        settings: VyperSettings,
        compilers_threads_semaphore: Arc<Semaphore>,
        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_variables)] extensions: Extensions,
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

        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_mut)]
        let mut client = VyperClient::new(compilers);

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
impl VyperVerifier for VyperVerifierService {
    async fn verify_multi_part(
        &self,
        request: Request<VerifyVyperMultiPartRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifyVyperMultiPartRequestWrapper = request.into_inner().into();
        let result = vyper::multi_part::verify(self.client.clone(), request.try_into()?).await;

        if let Ok(verification_success) = result {
            let response = VerifyResponseWrapper::ok(verification_success);
            metrics::count_verify_contract("vyper", response.status().as_str_name(), "multi-part");
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
