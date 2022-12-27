use crate::proto::{sourcify_verifier_server, VerifyResponse, VerifySourcifyRequest};
use async_trait::async_trait;

#[derive(Default)]
pub struct SourcifyVerifierService {}

#[async_trait]
impl sourcify_verifier_server::SourcifyVerifier for SourcifyVerifierService {
    async fn verify(
        &self,
        _request: tonic::Request<VerifySourcifyRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        todo!()
    }
}
