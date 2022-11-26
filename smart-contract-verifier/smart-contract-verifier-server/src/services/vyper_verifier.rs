use crate::settings::{FetcherSettings, VyperSettings};
use smart_contract_verifier::{Compilers, ListFetcher, VyperClient, VyperCompiler};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    vyper_verifier_server::VyperVerifier, ListVersionsRequest, ListVersionsResponse,
    VerifyResponse, VerifyVyperMultiPartRequest,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tonic::{Request, Response, Status};

pub struct VyperVerifierService {
    _client: Arc<VyperClient>,
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
            _client: Arc::new(client),
        })
    }
}

#[async_trait::async_trait]
impl VyperVerifier for VyperVerifierService {
    async fn verify_multi_part(
        &self,
        _request: Request<VerifyVyperMultiPartRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        todo!()
    }

    async fn list_versions(
        &self,
        _request: Request<ListVersionsRequest>,
    ) -> Result<Response<ListVersionsResponse>, Status> {
        todo!()
    }
}
