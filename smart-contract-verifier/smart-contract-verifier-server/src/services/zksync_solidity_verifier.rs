use crate::{
    proto::zksync::solidity::{
        verifier_server::Verifier, ListCompilersRequest, ListCompilersResponse, VerifyResponse,
        VerifyStandardJsonRequest,
    },
    services::common,
    settings::ZksyncSoliditySettings,
    types::{zksolc_standard_json::VerifyStandardJsonRequestWrapper, StandardJsonParseError},
};
use anyhow::Context;
use smart_contract_verifier::{
    zksync,
    zksync::{ZkSolcCompiler, ZkSyncCompilers},
    SolcValidator,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tonic::{Request, Response, Status};

pub struct Service {
    compilers: ZkSyncCompilers<ZkSolcCompiler>,
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
        .await
        .context("zksync zksolc fetcher initialization")?;

        let compilers = ZkSyncCompilers::new(
            evm_fetcher.clone(),
            zk_fetcher.clone(),
            compilers_threads_semaphore,
        );

        Ok(Self { compilers })
    }
}

#[async_trait::async_trait]
impl Verifier for Service {
    async fn verify_standard_json(
        &self,
        request: Request<VerifyStandardJsonRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifyStandardJsonRequestWrapper = request.into_inner().into();

        let verification_request: zksync::VerificationRequest = {
            let request: Result<_, StandardJsonParseError> = request.try_into();
            if let Err(err) = request {
                return match err {
                    StandardJsonParseError::InvalidContent(_) => {
                        // Ok(types::zksync_verification::compilation_error(format!(
                        //     "Invalid standard json: {err}"
                        // )))
                        todo!()
                    }
                    StandardJsonParseError::BadRequest(_) => {
                        tracing::info!(err=%err, "Bad request");
                        Err(Status::invalid_argument(err.to_string()))
                    }
                };
            }
            request.unwrap()
        };

        let result = zksync::verify(&self.compilers, verification_request).await;
        if let Err(err) = result {
            println!("[ERROR] {err:#}")
        }

        // match result {
        //     Ok(result) => types::zksync_verification::process_verification_result(result),
        //     Err(err) => types::zksync_verification::process_batch_error(err),
        // }

        Ok(Response::new(smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::zksync::solidity::VerifyResponse {
            verify_response: None,
        }))
    }

    async fn list_compilers(
        &self,
        _request: Request<ListCompilersRequest>,
    ) -> Result<Response<ListCompilersResponse>, Status> {
        Ok(Response::new(ListCompilersResponse {
            solc_compilers: self.compilers.all_evm_versions_sorted_str(),
            zk_compilers: self.compilers.all_zk_versions_sorted_str(),
        }))
    }
}
