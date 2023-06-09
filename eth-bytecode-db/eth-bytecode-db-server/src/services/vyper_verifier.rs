use super::verifier_base;
use crate::{
    proto::{
        vyper_verifier_server, ListCompilerVersionsRequest, ListCompilerVersionsResponse,
        VerifyResponse, VerifyVyperMultiPartRequest, VerifyVyperStandardJsonRequest,
    },
    types::{BytecodeTypeWrapper, VerificationMetadataWrapper},
};
use amplify::Wrapper;
use async_trait::async_trait;
use eth_bytecode_db::verification::{
    compiler_versions, vyper_multi_part, vyper_standard_json, Client, VerificationRequest,
};

pub struct VyperVerifierService {
    client: Client,
}

impl VyperVerifierService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl vyper_verifier_server::VyperVerifier for VyperVerifierService {
    async fn verify_multi_part(
        &self,
        request: tonic::Request<VerifyVyperMultiPartRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        let request = request.into_inner();

        let bytecode_type = request.bytecode_type();
        let verification_request = VerificationRequest {
            bytecode: request.bytecode,
            bytecode_type: BytecodeTypeWrapper::from_inner(bytecode_type).try_into()?,
            compiler_version: request.compiler_version,
            content: vyper_multi_part::MultiPartFiles {
                source_files: request.source_files,
                interfaces: request.interfaces,
                evm_version: request.evm_version,
            },
            metadata: request
                .metadata
                .map(|metadata| VerificationMetadataWrapper::from_inner(metadata).try_into())
                .transpose()?,
        };
        let result = vyper_multi_part::verify(self.client.clone(), verification_request).await;

        verifier_base::process_verification_result(result)
    }

    async fn verify_standard_json(
        &self,
        request: tonic::Request<VerifyVyperStandardJsonRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        let request = request.into_inner();

        let bytecode_type = request.bytecode_type();
        let verification_request = VerificationRequest {
            bytecode: request.bytecode,
            bytecode_type: BytecodeTypeWrapper::from_inner(bytecode_type).try_into()?,
            compiler_version: request.compiler_version,
            content: vyper_standard_json::StandardJson {
                input: request.input,
            },
            metadata: request
                .metadata
                .map(|metadata| VerificationMetadataWrapper::from_inner(metadata).try_into())
                .transpose()?,
        };
        let result = vyper_standard_json::verify(self.client.clone(), verification_request).await;

        verifier_base::process_verification_result(result)
    }

    async fn list_compiler_versions(
        &self,
        _request: tonic::Request<ListCompilerVersionsRequest>,
    ) -> Result<tonic::Response<ListCompilerVersionsResponse>, tonic::Status> {
        let result = compiler_versions::vyper_versions(self.client.clone()).await;

        verifier_base::process_compiler_versions_result(result)
    }
}
