use crate::proto::{
    solidity_verifier_server, ListCompilerVersionsRequest, ListCompilerVersionsResponse,
    VerifyResponse, VerifySolidityMultiPartRequest, VerifySolidityStandardJsonRequest,
};
use async_trait::async_trait;

#[derive(Default)]
pub struct SolidityVerifierService {}

#[async_trait]
impl solidity_verifier_server::SolidityVerifier for SolidityVerifierService {
    async fn verify_multi_part(
        &self,
        _request: tonic::Request<VerifySolidityMultiPartRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        todo!()
    }

    async fn verify_standard_json(
        &self,
        _request: tonic::Request<VerifySolidityStandardJsonRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        todo!()
    }

    async fn list_compiler_versions(
        &self,
        _request: tonic::Request<ListCompilerVersionsRequest>,
    ) -> Result<tonic::Response<ListCompilerVersionsResponse>, tonic::Status> {
        todo!()
    }
}
