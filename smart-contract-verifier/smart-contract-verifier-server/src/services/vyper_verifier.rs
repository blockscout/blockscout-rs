use crate::{
    metrics,
    proto::{
        vyper_verifier_server::VyperVerifier, BytecodeType, ListCompilerVersionsRequest,
        ListCompilerVersionsResponse, VerifyResponse, VerifyVyperMultiPartRequest,
        VerifyVyperStandardJsonRequest,
    },
    services::common,
    settings::VyperSettings,
    types,
    types::VerifyResponseWrapper,
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

        let client = VyperClient::new(compilers);

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
        let request = request.into_inner();

        let chain_id = request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.chain_id.clone());
        let contract_address = request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.contract_address.clone());
        tracing::info!(
            chain_id =? chain_id,
            contract_address =? contract_address,
            "vyper standard-json verification request received"
        );

        let maybe_verification_request = vyper::multi_part::VerificationRequest::try_from(request);
        let verification_request =
            common::process_solo_verification_request_conversion!(maybe_verification_request);

        let result = vyper::multi_part::verify(self.client.clone(), verification_request).await;

        let verify_response = match result {
            Ok(value) => types::verification_result::process_verification_result(value)?,
            Err(error) => types::verification_result::process_error(error)?,
        };

        metrics::count_verify_contract(
            &chain_id.unwrap_or_default(),
            "vyper",
            verify_response.status().as_str_name(),
            "multi-part",
        );
        Ok(Response::new(verify_response))

        // let response = if let Ok(verification_success) = result {
        //     tracing::info!(match_type=?verification_success.match_type, "Request processed successfully");
        //     VerifyResponseWrapper::ok(verification_success)
        // } else {
        //     let err = result.unwrap_err();
        //     tracing::info!(err=%err, "Request processing failed");
        //     match err {
        //         VerificationError::Compilation(_)
        //         | VerificationError::NoMatchingContracts
        //         | VerificationError::CompilerVersionMismatch(_) => VerifyResponseWrapper::err(err),
        //         VerificationError::Initialization(_) | VerificationError::VersionNotFound(_) => {
        //             return Err(Status::invalid_argument(err.to_string()));
        //         }
        //         VerificationError::Internal(err) => {
        //             tracing::error!("internal error: {err:#?}");
        //             return Err(Status::internal(err.to_string()));
        //         }
        //     }
        // };
    }

    async fn verify_standard_json(
        &self,
        request: Request<VerifyVyperStandardJsonRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request = request.into_inner();

        let chain_id = request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.chain_id.clone());
        let contract_address = request
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.contract_address.clone());
        tracing::info!(
            chain_id =? chain_id,
            contract_address =? contract_address,
            "vyper standard-json verification request received"
        );

        let maybe_verification_request =
            vyper::standard_json::VerificationRequest::try_from(request);
        let verification_request =
            common::process_solo_verification_request_conversion!(maybe_verification_request);

        let result = vyper::standard_json::verify(self.client.clone(), verification_request).await;

        let verify_response = match result {
            Ok(value) => types::verification_result::process_verification_result(value)?,
            Err(error) => types::verification_result::process_error(error)?,
        };

        metrics::count_verify_contract(
            &chain_id.unwrap_or_default(),
            "vyper",
            verify_response.status().as_str_name(),
            "standard-json",
        );
        Ok(Response::new(verify_response))
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
