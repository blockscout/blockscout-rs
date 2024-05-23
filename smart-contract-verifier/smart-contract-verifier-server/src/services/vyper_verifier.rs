use crate::{
    metrics,
    proto::{
        vyper_verifier_server::VyperVerifier, BytecodeType, ListCompilerVersionsRequest,
        ListCompilerVersionsResponse, VerifyResponse, VerifyVyperMultiPartRequest,
        VerifyVyperStandardJsonRequest,
    },
    services::common,
    settings::{Extensions, VyperSettings},
    types::{
        StandardJsonParseError, VerifyResponseWrapper, VerifyVyperMultiPartRequestWrapper,
        VerifyVyperStandardJsonRequestWrapper,
    },
};
use anyhow::Context;
use smart_contract_verifier::{vyper, Compilers, VerificationError, VyperClient, VyperCompiler};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tonic::{Request, Response, Status};

pub struct VyperVerifierService {
    client: Arc<VyperClient>,
}

impl VyperVerifierService {
    pub async fn new(
        settings: VyperSettings,
        compilers_threads_semaphore: Arc<Semaphore>,
        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_variables)] extensions: Extensions,
    ) -> anyhow::Result<Self> {
        let fetcher = common::initialize_fetcher(
            settings.fetcher,
            settings.compilers_dir.clone(),
            settings.refresh_versions_schedule,
            None,
        )
        .await
        .context("vyper fetcher initialization")?;
        let compilers = Compilers::new(fetcher, VyperCompiler::new(), compilers_threads_semaphore);
        compilers.load_from_dir(&settings.compilers_dir).await;

        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_mut)]
        let mut client = VyperClient::new(compilers);

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
impl VyperVerifier for VyperVerifierService {
    async fn verify_multi_part(
        &self,
        request: Request<VerifyVyperMultiPartRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifyVyperMultiPartRequestWrapper = request.into_inner().into();
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
            "Vyper multi-part verification request received"
        );

        tracing::debug!(
            bytecode = request.bytecode,
            bytecode_type = BytecodeType::from_i32(request.bytecode_type)
                .unwrap()
                .as_str_name(),
            compiler_version = request.compiler_version,
            evm_version = request.evm_version,
            source_files = ?request.source_files,
            interfaces = ?request.interfaces,
            "Request details"
        );

        let result = vyper::multi_part::verify(self.client.clone(), request.try_into()?).await;

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
            "vyper",
            response.status().as_str_name(),
            "multi-part",
        );
        return Ok(Response::new(response.into_inner()));
    }

    async fn verify_standard_json(
        &self,
        request: Request<VerifyVyperStandardJsonRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifyVyperStandardJsonRequestWrapper = request.into_inner().into();
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
            "Vyper standard-json verification request received"
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
                        return Ok(Response::new(response));
                    }
                    StandardJsonParseError::BadRequest(_) => {
                        return Err(Status::invalid_argument(err.to_string()));
                    }
                }
            }
            request.unwrap()
        };
        let result = vyper::standard_json::verify(self.client.clone(), verification_request).await;

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
            "vyper",
            response.status().as_str_name(),
            "standard-json",
        );
        return Ok(Response::new(response.into_inner()));
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
}
