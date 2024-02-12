use crate::proto::{
    vyper_verifier_server::VyperVerifier, ListCompilersRequest, ListCompilersResponse,
    VerificationResponse, VyperVerifyMultiPartRequest, VyperVerifyStandardJsonRequest,
};
use async_trait::async_trait;
use proxy_verifier_logic::{vyper_verifier_multi_part, vyper_verifier_standard_json};
use std::{collections::BTreeMap, sync::Arc};
use tonic::{Request, Response, Status};

pub struct VyperVerifierService {
    blockscout_clients: Arc<BTreeMap<String, blockscout_client::Client>>,
    eth_bytecode_db_client: Arc<eth_bytecode_db_proto::http_client::Client>,
}

impl VyperVerifierService {
    pub fn new(
        blockscout_clients: Arc<BTreeMap<String, blockscout_client::Client>>,
        eth_bytecode_db_client: Arc<eth_bytecode_db_proto::http_client::Client>,
    ) -> Self {
        Self {
            blockscout_clients,
            eth_bytecode_db_client,
        }
    }
}

#[async_trait]
impl VyperVerifier for VyperVerifierService {
    async fn verify_multi_part(
        &self,
        request: Request<VyperVerifyMultiPartRequest>,
    ) -> Result<Response<VerificationResponse>, Status> {
        let request = request.into_inner();
        let verification_request = vyper_verifier_multi_part::VerificationRequest {
            compiler: request.compiler,
            evm_version: request.evm_version,
            source_files: request.source_files,
            interfaces: request.interfaces,
        };

        super::verify(
            self.blockscout_clients.as_ref(),
            self.eth_bytecode_db_client.as_ref(),
            request.contracts,
            verification_request,
            vyper_verifier_multi_part::verify,
        )
        .await
    }

    async fn verify_standard_json(
        &self,
        request: Request<VyperVerifyStandardJsonRequest>,
    ) -> Result<Response<VerificationResponse>, Status> {
        let request = request.into_inner();
        let verification_request = vyper_verifier_standard_json::VerificationRequest {
            compiler: request.compiler,
            input: request.input,
        };

        super::verify(
            self.blockscout_clients.as_ref(),
            self.eth_bytecode_db_client.as_ref(),
            request.contracts,
            verification_request,
            vyper_verifier_standard_json::verify,
        )
        .await
    }

    async fn list_compilers(
        &self,
        _request: Request<ListCompilersRequest>,
    ) -> Result<Response<ListCompilersResponse>, Status> {
        super::list_compilers(
            self.eth_bytecode_db_client.as_ref(),
            eth_bytecode_db_proto::http_client::vyper_verifier_client::list_compiler_versions,
        )
        .await
    }
}
