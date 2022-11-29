use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    sourcify_verifier_server::SourcifyVerifier, VerifyResponse, VerifyViaSourcifyRequest,
};
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct SourcifyVerifierService {}

#[async_trait::async_trait]
impl SourcifyVerifier for SourcifyVerifierService {
    async fn verify(
        &self,
        _request: Request<VerifyViaSourcifyRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        todo!()
    }
}
