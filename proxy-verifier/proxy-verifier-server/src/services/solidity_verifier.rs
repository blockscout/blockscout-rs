use crate::proto::{
    solidity_verifier_server::SolidityVerifier, ListCompilersRequest, ListCompilersResponse,
    SolidityVerifyMultiPartRequest, SolidityVerifyStandardJsonRequest, VerificationResponse,
};
use async_trait::async_trait;
use proxy_verifier_logic::{solidity_verifier_multi_part, solidity_verifier_standard_json};
use std::{collections::BTreeMap, sync::Arc};
use tonic::{Request, Response, Status};

pub struct SolidityVerifierService {
    blockscout_clients: Arc<BTreeMap<String, proxy_verifier_logic::blockscout::Client>>,
    eth_bytecode_db_client: Arc<eth_bytecode_db_proto::http_client::Client>,
}

impl SolidityVerifierService {
    pub fn new(
        blockscout_clients: Arc<BTreeMap<String, proxy_verifier_logic::blockscout::Client>>,
        eth_bytecode_db_client: Arc<eth_bytecode_db_proto::http_client::Client>,
    ) -> Self {
        Self {
            blockscout_clients,
            eth_bytecode_db_client,
        }
    }
}

#[async_trait]
impl SolidityVerifier for SolidityVerifierService {
    async fn verify_multi_part(
        &self,
        request: Request<SolidityVerifyMultiPartRequest>,
    ) -> Result<Response<VerificationResponse>, Status> {
        let request = request.into_inner();
        let verification_request = solidity_verifier_multi_part::VerificationRequest {
            compiler: request.compiler,
            evm_version: request.evm_version,
            optimization_runs: request.optimization_runs,
            source_files: request.source_files,
            libraries: request.libraries,
        };

        super::verify(
            self.blockscout_clients.as_ref(),
            self.eth_bytecode_db_client.as_ref(),
            request.contracts,
            verification_request,
            solidity_verifier_multi_part::verify,
        )
        .await
    }

    async fn verify_standard_json(
        &self,
        request: Request<SolidityVerifyStandardJsonRequest>,
    ) -> Result<Response<VerificationResponse>, Status> {
        let request = request.into_inner();
        let verification_request = solidity_verifier_standard_json::VerificationRequest {
            compiler: request.compiler,
            input: request.input,
        };

        super::verify(
            self.blockscout_clients.as_ref(),
            self.eth_bytecode_db_client.as_ref(),
            request.contracts,
            verification_request,
            solidity_verifier_standard_json::verify,
        )
        .await
    }

    async fn list_compilers(
        &self,
        _request: Request<ListCompilersRequest>,
    ) -> Result<Response<ListCompilersResponse>, Status> {
        let compilers = super::list_compilers(
            self.eth_bytecode_db_client.as_ref(),
            eth_bytecode_db_proto::http_client::solidity_verifier_client::list_compiler_versions,
            super::SOLIDITY_EVM_VERSIONS,
        )
        .await?;

        Ok(Response::new(ListCompilersResponse { compilers }))
    }
}
