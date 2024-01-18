use super::verifier_base;
use crate::{
    proto::{
        solidity_verifier_server, ListCompilerVersionsRequest, ListCompilerVersionsResponse,
        VerifyResponse, VerifySolidityMultiPartRequest, VerifySolidityStandardJsonRequest,
    },
    types::{BytecodeTypeWrapper, VerificationMetadataWrapper},
};
use amplify::Wrapper;
use async_trait::async_trait;
use eth_bytecode_db::verification::{
    compiler_versions, solidity_multi_part, solidity_standard_json, Client, VerificationRequest,
};
use std::collections::HashSet;
use tracing::instrument;

pub struct SolidityVerifierService {
    client: Client,
    authorized_keys: HashSet<String>,
}

impl SolidityVerifierService {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            authorized_keys: Default::default(),
        }
    }

    pub fn with_authorized_keys(mut self, authorized_keys: HashSet<String>) -> Self {
        self.authorized_keys = authorized_keys;
        self
    }
}

#[async_trait]
impl solidity_verifier_server::SolidityVerifier for SolidityVerifierService {
    #[instrument(skip_all)]
    async fn verify_multi_part(
        &self,
        request: tonic::Request<VerifySolidityMultiPartRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        let (metadata, _, request) = request.into_parts();
        super::trace_verification_request!(&request);

        let is_authorized = super::is_key_authorized(&self.authorized_keys, metadata)?;
        tracing::info!(is_authorized = is_authorized);

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
            metadata: request
                .metadata
                .map(|metadata| VerificationMetadataWrapper::from_inner(metadata).try_into())
                .transpose()?,
            is_authorized,
        };
        let result = solidity_multi_part::verify(self.client.clone(), verification_request).await;

        verifier_base::process_verification_result(result)
    }

    #[instrument(skip_all)]
    async fn verify_standard_json(
        &self,
        request: tonic::Request<VerifySolidityStandardJsonRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        let (metadata, _, request) = request.into_parts();
        super::trace_verification_request!(&request);

        let is_authorized = super::is_key_authorized(&self.authorized_keys, metadata)?;
        tracing::info!(is_authorized = is_authorized);

        let bytecode_type = request.bytecode_type();
        let verification_request = VerificationRequest {
            bytecode: request.bytecode,
            bytecode_type: BytecodeTypeWrapper::from_inner(bytecode_type).try_into()?,
            compiler_version: request.compiler_version,
            content: solidity_standard_json::StandardJson {
                input: request.input,
            },
            metadata: request
                .metadata
                .map(|metadata| VerificationMetadataWrapper::from_inner(metadata).try_into())
                .transpose()?,
            is_authorized,
        };
        let result =
            solidity_standard_json::verify(self.client.clone(), verification_request).await;

        verifier_base::process_verification_result(result)
    }

    #[instrument(skip_all)]
    async fn list_compiler_versions(
        &self,
        _request: tonic::Request<ListCompilerVersionsRequest>,
    ) -> Result<tonic::Response<ListCompilerVersionsResponse>, tonic::Status> {
        let result = compiler_versions::solidity_versions(self.client.clone()).await;

        verifier_base::process_compiler_versions_result(result)
    }
}
