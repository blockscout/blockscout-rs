use super::verifier_base;
use crate::proto::{sourcify_verifier_server, VerifyResponse, VerifySourcifyRequest};
use async_trait::async_trait;
use eth_bytecode_db::verification::{
    sourcify::{self, VerificationRequest},
    Client,
};

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

        let verification_request = VerificationRequest {
            address: request.address,
            chain: request.chain,
            chosen_contract: request.chosen_contract,
            source_files: request.files,
        };

        let result = sourcify::verify(self.client.clone(), verification_request).await;

        verifier_base::process_verification_result(result)
    }
}
