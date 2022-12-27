use crate::proto::{
    vyper_verifier_server, ListCompilerVersionsRequest, ListCompilerVersionsResponse,
    VerifyResponse, VerifyVyperMultiPartRequest,
};
use async_trait::async_trait;

#[derive(Default)]
pub struct VyperVerifierService {}

#[async_trait]
impl vyper_verifier_server::VyperVerifier for VyperVerifierService {
    async fn verify_multi_part(
        &self,
        _request: tonic::Request<VerifyVyperMultiPartRequest>,
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
