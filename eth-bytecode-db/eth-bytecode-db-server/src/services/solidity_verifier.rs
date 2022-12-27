use crate::{
    proto::{
        solidity_verifier_server, ListCompilerVersionsRequest, ListCompilerVersionsResponse,
        VerifyResponse, VerifySolidityMultiPartRequest, VerifySolidityStandardJsonRequest,
    },
    types::{BytecodeTypeWrapper, VerifyResponseWrapper},
};
use amplify::Wrapper;
use async_trait::async_trait;
use eth_bytecode_db::verification::{
    solidity_multi_part, solidity_standard_json, Client, Error, VerificationRequest,
};

pub struct SolidityVerifierService {
    client: Client,
}

impl Default for SolidityVerifierService {
    fn default() -> Self {
        todo!()
    }
}

#[async_trait]
impl solidity_verifier_server::SolidityVerifier for SolidityVerifierService {
    async fn verify_multi_part(
        &self,
        request: tonic::Request<VerifySolidityMultiPartRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        let request = request.into_inner();

        let bytecode_type = request.bytecode_type();
        let verification_request = VerificationRequest {
            bytecode: request.bytecode,
            bytecode_type: BytecodeTypeWrapper::from_inner(bytecode_type).try_into()?,
            compiler_version: request.compiler_version,
            content: solidity_multi_part::MultiPartFiles {
                source_files: request.source_files,
                evm_version: request.evm_version,
                optimization_runs: request.optimization_runs,
                libraries: request.libraries,
            },
        };
        let result = solidity_multi_part::verify(self.client.clone(), verification_request).await;

        match result {
            Ok(source) => {
                let response = VerifyResponseWrapper::ok(source);
                Ok(tonic::Response::new(response.into()))
            }
            Err(Error::VerificationFailed { message }) => {
                let response = VerifyResponseWrapper::err(message);
                Ok(tonic::Response::new(response.into()))
            }
            Err(Error::InvalidArgument(message)) => Err(tonic::Status::invalid_argument(message)),
            Err(Error::Internal(message)) => Err(tonic::Status::internal(message.to_string())),
        }
    }

    async fn verify_standard_json(
        &self,
        request: tonic::Request<VerifySolidityStandardJsonRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        let request = request.into_inner();

        let bytecode_type = request.bytecode_type();
        let verification_request = VerificationRequest {
            bytecode: request.bytecode,
            bytecode_type: BytecodeTypeWrapper::from_inner(bytecode_type).try_into()?,
            compiler_version: request.compiler_version,
            content: solidity_standard_json::StandardJson {
                input: request.input,
            },
        };
        let result =
            solidity_standard_json::verify(self.client.clone(), verification_request).await;

        match result {
            Ok(source) => {
                let response = VerifyResponseWrapper::ok(source);
                Ok(tonic::Response::new(response.into()))
            }
            Err(Error::VerificationFailed { message }) => {
                let response = VerifyResponseWrapper::err(message);
                Ok(tonic::Response::new(response.into()))
            }
            Err(Error::InvalidArgument(message)) => Err(tonic::Status::invalid_argument(message)),
            Err(Error::Internal(message)) => Err(tonic::Status::internal(message.to_string())),
        }
    }

    async fn list_compiler_versions(
        &self,
        _request: tonic::Request<ListCompilerVersionsRequest>,
    ) -> Result<tonic::Response<ListCompilerVersionsResponse>, tonic::Status> {
        todo!()
    }
}
