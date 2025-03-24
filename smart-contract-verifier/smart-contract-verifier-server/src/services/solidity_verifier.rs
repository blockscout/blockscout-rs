use crate::{
    metrics,
    proto::{
        solidity_verifier_server::SolidityVerifier, BatchVerifyResponse,
        BatchVerifySolidityMultiPartRequest, BatchVerifySolidityStandardJsonRequest,
        ListCompilerVersionsRequest, ListCompilerVersionsResponse, VerifyResponse,
        VerifySolidityMultiPartRequest, VerifySolidityStandardJsonRequest,
    },
    services::common,
    settings::SoliditySettings,
    types,
    types::{LookupMethodsRequestWrapper, LookupMethodsResponseWrapper, VerifyResponseWrapper},
};
use anyhow::Context;
use smart_contract_verifier::{
    find_methods, solidity, solidity::RequestParseError, Compilers, SolcValidator, SolidityClient,
    SolidityCompiler,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    LookupMethodsRequest, LookupMethodsResponse,
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

        let client = SolidityClient::new(compilers);

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
            "solidity multi-part verification request received"
        );

        let maybe_verification_request =
            solidity::multi_part::VerificationRequest::try_from(request);
        let verification_request = match maybe_verification_request {
            Ok(request) => request,
            Err(err @ RequestParseError::InvalidContent(_)) => {
                let response = VerifyResponseWrapper::err(err).into_inner();
                tracing::info!(response=?response, "request processed");
                return Ok(Response::new(response));
            }
            Err(err @ RequestParseError::BadRequest(_)) => {
                tracing::info!(err=%err, "bad request");
                return Err(Status::invalid_argument(err.to_string()));
            }
        };

        let result = solidity::multi_part::verify(self.client.clone(), verification_request).await;

        let verify_response = match result {
            Ok(value) => types::verification_result::process_verification_result(value)?,
            Err(error) => types::verification_result::process_error(error)?,
        };

        metrics::count_verify_contract(
            &chain_id.unwrap_or_default(),
            "solidity",
            verify_response.status().as_str_name(),
            "multi-part",
        );
        Ok(Response::new(verify_response))
    }

    async fn verify_standard_json(
        &self,
        request: Request<VerifySolidityStandardJsonRequest>,
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
            "solidity standard-json verification request received"
        );

        let maybe_verification_request =
            solidity::standard_json::VerificationRequest::try_from(request);
        let verification_request = match maybe_verification_request {
            Ok(request) => request,
            Err(err @ RequestParseError::InvalidContent(_)) => {
                let response = VerifyResponseWrapper::err(err).into_inner();
                tracing::info!(response=?response, "request processed");
                return Ok(Response::new(response));
            }
            Err(err @ RequestParseError::BadRequest(_)) => {
                tracing::info!(err=%err, "bad request");
                return Err(Status::invalid_argument(err.to_string()));
            }
        };

        let result =
            solidity::standard_json::verify(self.client.clone(), verification_request).await;

        let verify_response = match result {
            Ok(value) => types::verification_result::process_verification_result(value)?,
            Err(error) => types::verification_result::process_error(error)?,
        };

        metrics::count_verify_contract(
            &chain_id.unwrap_or_default(),
            "solidity",
            verify_response.status().as_str_name(),
            "standard-json",
        );
        Ok(Response::new(verify_response))
    }

    async fn batch_verify_multi_part(
        &self,
        request: Request<BatchVerifySolidityMultiPartRequest>,
    ) -> Result<Response<BatchVerifyResponse>, Status> {
        let request = request.into_inner();

        let maybe_verification_request =
            solidity::multi_part::BatchVerificationRequest::try_from(request);
        let verification_request = match maybe_verification_request {
            Ok(request) => request,
            Err(err @ RequestParseError::InvalidContent(_)) => {
                let response = types::batch_verification::compilation_error(err.to_string());
                tracing::info!(response=?response, "request processed");
                return Ok(Response::new(response));
            }
            Err(err @ RequestParseError::BadRequest(_)) => {
                tracing::info!(err=%err, "bad request");
                return Err(Status::invalid_argument(err.to_string()));
            }
        };

        let result =
            solidity::multi_part::batch_verify(self.client.clone(), verification_request).await;

        let verify_response = match result {
            Ok(value) => types::batch_verification::process_verification_results(value)?,
            Err(error) => types::batch_verification::process_error(error)?,
        };

        Ok(Response::new(verify_response))
    }

    async fn batch_verify_standard_json(
        &self,
        request: Request<BatchVerifySolidityStandardJsonRequest>,
    ) -> Result<Response<BatchVerifyResponse>, Status> {
        let request = request.into_inner();

        let maybe_verification_request =
            solidity::standard_json::BatchVerificationRequest::try_from(request);
        let verification_request = match maybe_verification_request {
            Ok(request) => request,
            Err(err @ RequestParseError::InvalidContent(_)) => {
                let response = types::batch_verification::compilation_error(err.to_string());
                tracing::info!(response=?response, "request processed");
                return Ok(Response::new(response));
            }
            Err(err @ RequestParseError::BadRequest(_)) => {
                tracing::info!(err=%err, "bad request");
                return Err(Status::invalid_argument(err.to_string()));
            }
        };

        let result =
            solidity::standard_json::batch_verify(self.client.clone(), verification_request).await;

        let verify_response = match result {
            Ok(value) => types::batch_verification::process_verification_results(value)?,
            Err(error) => types::batch_verification::process_error(error)?,
        };

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
