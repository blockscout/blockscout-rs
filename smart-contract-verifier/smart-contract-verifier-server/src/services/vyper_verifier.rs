use crate::{
    metrics,
    settings::{FetcherSettings, VyperSettings},
    types::{VerifyResponseWrapper, VerifyVyperMultiPartRequestWrapper},
};
use smart_contract_verifier::{
    vyper, Compilers, ListFetcher, VerificationError, VyperClient, VyperCompiler,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    vyper_verifier_server::VyperVerifier, ListVersionsRequest, ListVersionsResponse,
    VerifyResponse, VerifyVyperMultiPartRequest,
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
        let client = VyperClient::new(compilers);
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
            let response = VerifyResponseWrapper::ok(verification_success.into());
            metrics::count_verify_contract("vyper", &response.status, "multi-part");
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

    async fn list_versions(
        &self,
        _request: Request<ListVersionsRequest>,
    ) -> Result<Response<ListVersionsResponse>, Status> {
        let versions = self.client.compilers().all_versions_sorted_str();
        Ok(Response::new(ListVersionsResponse { versions }))
    }
}
