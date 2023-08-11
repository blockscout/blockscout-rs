use super::verifier_base;
use crate::proto::{
    sourcify_verifier_server, VerifyFromEtherscanSourcifyRequest, VerifyResponse,
    VerifySourcifyRequest,
};
use async_trait::async_trait;
use eth_bytecode_db::verification::{sourcify, sourcify_from_etherscan, Client};

pub struct SourcifyVerifierService {
    client: Client,
}

impl SourcifyVerifierService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl sourcify_verifier_server::SourcifyVerifier for SourcifyVerifierService {
    async fn verify(
        &self,
        request: tonic::Request<VerifySourcifyRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        let request = request.into_inner();

        let verification_request = sourcify::VerificationRequest {
            address: request.address,
            chain: request.chain,
            chosen_contract: request.chosen_contract,
            source_files: request.files,
        };

        let result = sourcify::verify(self.client.clone(), verification_request).await;

        verifier_base::process_verification_result(result)
    }

    async fn verify_from_etherscan(
        &self,
        request: tonic::Request<VerifyFromEtherscanSourcifyRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        let request = request.into_inner();

        let verification_request = sourcify_from_etherscan::VerificationRequest {
            address: request.address,
            chain: request.chain,
        };

        let result =
            sourcify_from_etherscan::verify(self.client.clone(), verification_request).await;

        verifier_base::process_verification_result(result)
    }
}
