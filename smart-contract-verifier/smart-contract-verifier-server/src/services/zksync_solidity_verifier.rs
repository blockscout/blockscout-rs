use crate::{
    proto::zksync::solidity::{
        verifier_server::Verifier, ListCompilersRequest, ListCompilersResponse, VerifyResponse,
        VerifyStandardJsonRequest,
    },
    services::common,
    settings::{FetcherSettings, ZksyncSoliditySettings},
};
use anyhow::Context;
use smart_contract_verifier::{Fetcher, ListFetcher, S3Fetcher, SolcValidator, ZksyncCompilers};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tonic::{Request, Response, Status};

pub struct Service {
    compilers: ZksyncCompilers,
}

impl Service {
    pub async fn new(
        settings: ZksyncSoliditySettings,
        compilers_threads_semaphore: Arc<Semaphore>,
    ) -> anyhow::Result<Self> {
        let solc_validator = Arc::new(SolcValidator::default());
        let evm_fetcher = common::initialize_fetcher(
            settings.evm_fetcher,
            settings.evm_compilers_dir.clone(),
            settings.evm_refresh_versions_schedule,
            Some(solc_validator),
        )
        .await
        .context("zksync solc fetcher initialization")?;

        let zk_fetcher = common::initialize_fetcher(
            settings.zk_fetcher,
            settings.zk_compilers_dir.clone(),
            settings.zk_refresh_versions_schedule,
            None,
        )
            .await.context("zksync zksolc fetcher initialization")?;

        let compilers = ZksyncCompilers::new(evm_fetcher.clone(), zk_fetcher.clone());

        Ok(Self { compilers })
    }
}

#[async_trait::async_trait]
impl Verifier for Service {
    async fn verify_standard_json(
        &self,
        request: Request<VerifyStandardJsonRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        todo!()
    }

    async fn list_compilers(
        &self,
        request: Request<ListCompilersRequest>,
    ) -> Result<Response<ListCompilersResponse>, Status> {
        Ok(Response::new(ListCompilersResponse {
            zk_compilers: self.compilers.all_evm_versions_sorted_str(),
            solc_compilers: self.compilers.all_zk_versions_sorted_str(),
        }))
    }
}
