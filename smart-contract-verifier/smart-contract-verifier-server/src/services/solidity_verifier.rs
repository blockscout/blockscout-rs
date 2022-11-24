use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    solidity_verifier_server::SolidityVerifier, ListVersionsRequest, VerifyResponse,
    VerifySolidityMultiPartRequest, VerifySolidityStandardJsonRequest, Versions,
};
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct SolidityVerifierService {}

#[async_trait::async_trait]
impl SolidityVerifier for SolidityVerifierService {
    async fn verify_multi_part(
        &self,
        _request: Request<VerifySolidityMultiPartRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        todo!()
    }

    async fn verify_standard_json(
        &self,
        _request: Request<VerifySolidityStandardJsonRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        todo!()
    }

    async fn list_versions(
        &self,
        _request: Request<ListVersionsRequest>,
    ) -> Result<Response<Versions>, Status> {
        todo!()
    }
}
