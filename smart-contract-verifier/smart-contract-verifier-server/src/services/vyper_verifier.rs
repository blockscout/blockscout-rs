use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    vyper_verifier_server::VyperVerifier, ListVersionsRequest, ListVersionsResponse,
    VerifyResponse, VerifyVyperMultiPartRequest,
};
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct VyperVerifierService {}

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
