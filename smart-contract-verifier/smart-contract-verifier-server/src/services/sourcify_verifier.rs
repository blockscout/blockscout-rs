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
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};

pub struct SourcifyVerifierService {
    client: Arc<SourcifyApiClient>,
    lib_client: Arc<sourcify::Client>,
}

impl SourcifyVerifierService {
    pub async fn new(
        settings: SourcifySettings, /* Otherwise, results in compilation warning if all extensions are disabled */
        #[allow(unused_variables)] extensions: Extensions,
    ) -> anyhow::Result<Self> {
        let total_duration =
            settings.request_timeout * settings.verification_attempts.get() as u64 * 2;
        let lib_client = sourcify::ClientBuilder::default()
            .try_base_url(settings.api_url.as_str())
            .unwrap()
            .total_duration(Duration::from_secs(total_duration))
            .build();

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
            lib_client: Arc::new(lib_client),
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

        let chain_id = request.chain.clone();

        let result = process_verification_result(
            sc_sourcify::api::verify_from_etherscan(self.lib_client.clone(), request.try_into()?)
                .await,
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
