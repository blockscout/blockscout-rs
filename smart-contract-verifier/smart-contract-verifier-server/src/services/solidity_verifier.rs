use crate::{
    metrics,
    proto::{
        solidity_verifier_server::SolidityVerifier, BatchVerifyResponse,
        BatchVerifySolidityMultiPartRequest, BatchVerifySolidityStandardJsonRequest,
        ListCompilerVersionsRequest, ListCompilerVersionsResponse, VerifyResponse,
        VerifySolidityMultiPartRequest, VerifySolidityStandardJsonRequest,
    },
    services::common,
    settings::{Extensions, SoliditySettings},
    types,
    types::{
        LookupMethodsRequestWrapper, LookupMethodsResponseWrapper, StandardJsonParseError,
        VerifyResponseWrapper, VerifySolidityMultiPartRequestWrapper,
        VerifySolidityStandardJsonRequestWrapper,
    },
};
use anyhow::Context;
use smart_contract_verifier::{
    find_methods, solidity, Compilers, SolcValidator, SolidityClient, SolidityCompiler,
    VerificationError,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    BytecodeType, LookupMethodsRequest, LookupMethodsResponse,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tonic::{Request, Response, Status};

pub struct SolidityVerifierService {
    client: Arc<SolidityClient>,
}

impl SolidityVerifierService {
    pub async fn new(
        settings: SoliditySettings,
        compilers_threads_semaphore: Arc<Semaphore>,
        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_variables)] extensions: Extensions,
    ) -> anyhow::Result<Self> {
        let solc_validator = Arc::new(SolcValidator::default());
        let fetcher = common::initialize_fetcher(
            settings.fetcher,
            settings.compilers_dir.clone(),
            settings.refresh_versions_schedule,
            Some(solc_validator),
        )
        .await
        .context("solidity fetcher initialization")?;
        let compilers = Compilers::new(
            fetcher,
            SolidityCompiler::new(),
            compilers_threads_semaphore,
        );
        compilers.load_from_dir(&settings.compilers_dir).await;

        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_mut)]
        let mut client = SolidityClient::new(compilers);

        #[cfg(feature = "sig-provider-extension")]
        if let Some(sig_provider) = extensions.sig_provider {
            // TODO(#221): create only one instance of middleware/connection
            client = client
                .with_middleware(sig_provider_extension::SigProvider::new(sig_provider).await?);
        }

        Ok(Self {
            client: Arc::new(client),
        })
    }
}

#[async_trait::async_trait]
impl SolidityVerifier for SolidityVerifierService {
    async fn verify_multi_part(
        &self,
        request: Request<VerifySolidityMultiPartRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifySolidityMultiPartRequestWrapper = request.into_inner().into();
        let chain_id = request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.chain_id.clone())
            .unwrap_or_default();
        let contract_address = request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.contract_address.clone())
            .unwrap_or_default();
        tracing::info!(
            chain_id = chain_id,
            contract_address = contract_address,
            "Solidity multi-part verification request received"
        );

        tracing::debug!(
            bytecode = request.bytecode,
            bytecode_type = BytecodeType::from_i32(request.bytecode_type)
                .unwrap()
                .as_str_name(),
            compiler_version = request.compiler_version,
            evm_version = request.evm_version,
            optimization_runs = request.optimization_runs,
            source_files = ?request.source_files,
            libraries = ?request.libraries,
            "Request details"
        );

        let result = solidity::multi_part::verify(self.client.clone(), request.try_into()?).await;

        let response = if let Ok(verification_success) = result {
            tracing::info!(match_type=?verification_success.match_type, "Request processed successfully");
            VerifyResponseWrapper::ok(verification_success)
        } else {
            let err = result.unwrap_err();
            tracing::info!(err=%err, "Request processing failed");
            match err {
                VerificationError::Compilation(_)
                | VerificationError::NoMatchingContracts
                | VerificationError::CompilerVersionMismatch(_) => VerifyResponseWrapper::err(err),
                VerificationError::Initialization(_) | VerificationError::VersionNotFound(_) => {
                    return Err(Status::invalid_argument(err.to_string()));
                }
                VerificationError::Internal(err) => {
                    tracing::error!("internal error: {err:#?}");
                    return Err(Status::internal(err.to_string()));
                }
            }
        };

        metrics::count_verify_contract(
            chain_id.as_ref(),
            "solidity",
            response.status().as_str_name(),
            "multi-part",
        );
        Ok(Response::new(response.into_inner()))
    }

    async fn verify_standard_json(
        &self,
        request: Request<VerifySolidityStandardJsonRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifySolidityStandardJsonRequestWrapper = request.into_inner().into();
        let chain_id = request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.chain_id.clone())
            .unwrap_or_default();
        let contract_address = request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.contract_address.clone())
            .unwrap_or_default();
        tracing::info!(
            chain_id = chain_id,
            contract_address = contract_address,
            "Solidity standard-json verification request received"
        );

        tracing::debug!(
            bytecode = request.bytecode,
            bytecode_type = BytecodeType::from_i32(request.bytecode_type)
                .unwrap()
                .as_str_name(),
            compiler_version = request.compiler_version,
            input = request.input,
            "Request details"
        );

        let verification_request = {
            let request: Result<_, StandardJsonParseError> = request.try_into();
            if let Err(err) = request {
                match err {
                    StandardJsonParseError::InvalidContent(_) => {
                        let response = VerifyResponseWrapper::err(err).into_inner();
                        tracing::info!(response=?response, "Request processed");
                        return Ok(Response::new(response));
                    }
                    StandardJsonParseError::BadRequest(_) => {
                        tracing::info!(err=%err, "Bad request");
                        return Err(Status::invalid_argument(err.to_string()));
                    }
                }
            }
            request.unwrap()
        };
        let result =
            solidity::standard_json::verify(self.client.clone(), verification_request).await;

        let response = if let Ok(verification_success) = result {
            tracing::info!(match_type=?verification_success.match_type, "Request processed successfully");
            VerifyResponseWrapper::ok(verification_success)
        } else {
            let err = result.unwrap_err();
            tracing::info!(err=%err, "Request processing failed");
            match err {
                VerificationError::Compilation(_)
                | VerificationError::NoMatchingContracts
                | VerificationError::CompilerVersionMismatch(_) => VerifyResponseWrapper::err(err),
                VerificationError::Initialization(_) | VerificationError::VersionNotFound(_) => {
                    return Err(Status::invalid_argument(err.to_string()));
                }
                VerificationError::Internal(err) => {
                    tracing::error!("internal error: {err:#?}");
                    return Err(Status::internal(err.to_string()));
                }
            }
        };

        metrics::count_verify_contract(
            chain_id.as_ref(),
            "solidity",
            response.status().as_str_name(),
            "standard-json",
        );
        Ok(Response::new(response.into_inner()))
    }

    async fn batch_verify_multi_part(
        &self,
        request: Request<BatchVerifySolidityMultiPartRequest>,
    ) -> Result<Response<BatchVerifyResponse>, Status> {
        let request = request.into_inner();

        let contracts =
            types::batch_verification::from_proto_contracts_to_inner(&request.contracts)?;
        let compiler_version = types::batch_verification::from_proto_compiler_version_to_inner(
            &request.compiler_version,
        )?;

        let content = types::batch_verification::from_proto_solidity_multi_part_content_to_inner(
            request.sources,
            request.evm_version,
            request.optimization_runs,
            request.libraries,
        )?;

        let verification_request = solidity::multi_part::BatchVerificationRequest {
            contracts,
            compiler_version,
            content,
        };

        let result =
            solidity::multi_part::batch_verify(self.client.clone(), verification_request).await;

        match result {
            Ok(results) => types::batch_verification::process_verification_results(results),
            Err(err) => types::batch_verification::process_batch_error(err),
        }
    }

    async fn batch_verify_standard_json(
        &self,
        request: Request<BatchVerifySolidityStandardJsonRequest>,
    ) -> Result<Response<BatchVerifyResponse>, Status> {
        let request = request.into_inner();

        let contracts =
            types::batch_verification::from_proto_contracts_to_inner(&request.contracts)?;
        let compiler_version = types::batch_verification::from_proto_compiler_version_to_inner(
            &request.compiler_version,
        )?;

        let input = match serde_json::from_str::<foundry_compilers::CompilerInput>(&request.input) {
            Ok(input) => input,
            Err(err) => {
                return Ok(types::batch_verification::compilation_error(format!(
                    "Invalid standard json: {err}"
                )))
            }
        };

        let verification_request = solidity::standard_json::BatchVerificationRequest {
            contracts,
            compiler_version,
            content: solidity::standard_json::StandardJsonContent { input },
        };

        let result =
            solidity::standard_json::batch_verify(self.client.clone(), verification_request).await;

        match result {
            Ok(results) => types::batch_verification::process_verification_results(results),
            Err(err) => types::batch_verification::process_batch_error(err),
        }
    }

    async fn list_compiler_versions(
        &self,
        _request: Request<ListCompilerVersionsRequest>,
    ) -> Result<Response<ListCompilerVersionsResponse>, Status> {
        let compiler_versions = self.client.compilers().all_versions_sorted_str();
        Ok(Response::new(ListCompilerVersionsResponse {
            compiler_versions,
        }))
    }

    async fn lookup_methods(
        &self,
        request: Request<LookupMethodsRequest>,
    ) -> Result<Response<LookupMethodsResponse>, Status> {
        let request: LookupMethodsRequestWrapper = request.into_inner().into();
        let methods = find_methods(request.try_into()?);
        let response = LookupMethodsResponseWrapper::from(methods);
        Ok(Response::new(response.into()))
    }
}
