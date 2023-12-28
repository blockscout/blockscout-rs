use crate::{
    metrics,
    proto::{
        sourcify_verifier_server::SourcifyVerifier, VerifyFromEtherscanSourcifyRequest,
        VerifyResponse, VerifySourcifyRequest,
    },
    settings::{Extensions, SourcifySettings},
    types::{
        VerifyFromEtherscanSourcifyRequestWrapper, VerifyResponseWrapper,
        VerifySourcifyRequestWrapper,
    },
};
use smart_contract_verifier::{sourcify as sc_sourcify, sourcify::Error, SourcifyApiClient};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct SourcifyVerifierService {
    client: Arc<SourcifyApiClient>,
}

impl SourcifyVerifierService {
    pub async fn new(
        settings: SourcifySettings, /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_variables)] extensions: Extensions,
    ) -> anyhow::Result<Self> {
        /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_mut)]
        let mut client = {
            SourcifyApiClient::new(
                settings.api_url,
                settings.request_timeout,
                settings.verification_attempts,
            )
            .expect("failed to build sourcify client")
        };

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
impl SourcifyVerifier for SourcifyVerifierService {
    async fn verify(
        &self,
        request: Request<VerifySourcifyRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifySourcifyRequestWrapper = request.into_inner().into();

        tracing::info!(
            chain_id = request.chain,
            contract_address = request.address,
            "Sourcify verification request received"
        );

        tracing::debug!(
            files = ?request.files,
            chosen_contract = request.chosen_contract,
            "Request details"
        );

        let chain_id = request.chain.clone();

        let result = process_verification_result(
            sc_sourcify::api::verify(self.client.clone(), request.try_into()?).await,
        )?;

        metrics::count_verify_contract(
            chain_id.as_ref(),
            "solidity",
            result.status().as_str_name(),
            "sourcify",
        );
        return Ok(Response::new(result.into_inner()));
    }

    async fn verify_from_etherscan(
        &self,
        request: Request<VerifyFromEtherscanSourcifyRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifyFromEtherscanSourcifyRequestWrapper = request.into_inner().into();

        tracing::info!(
            chain_id = request.chain,
            contract_address = request.address,
            "Sourcify verification via etherscan request received"
        );

        let chain_id = request.chain.clone();

        let result = process_verification_result(
            sc_sourcify::api::verify_from_etherscan(self.client.clone(), request.try_into()?).await,
        )?;

        metrics::count_verify_contract(
            chain_id.as_ref(),
            "solidity",
            result.status().as_str_name(),
            "sourcify-from-etherscan",
        );
        return Ok(Response::new(result.into_inner()));
    }
}

fn process_verification_result(
    response: Result<sc_sourcify::Success, Error>,
) -> Result<VerifyResponseWrapper, Status> {
    match response {
        Ok(verification_success) => Ok(VerifyResponseWrapper::ok(verification_success)),
        Err(err) => match err {
            Error::Internal(err) => {
                tracing::error!("internal error: {err:#?}");
                Err(Status::internal(err.to_string()))
            }
            Error::BadRequest(err) => {
                tracing::error!("bad request error: {err:#?}");
                Err(Status::invalid_argument(err.to_string()))
            }
            Error::Verification(err) => Ok(VerifyResponseWrapper::err(err)),
            Error::Validation(err) => {
                tracing::debug!("invalid argument: {err:#?}");
                Err(Status::invalid_argument(err))
            }
        },
    }
}
