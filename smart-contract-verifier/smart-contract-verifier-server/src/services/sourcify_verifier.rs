use crate::{
    metrics,
    settings::{Extensions, SourcifySettings},
    types::{VerifyResponseWrapper, VerifyViaSourcifyRequestWrapper},
};
use smart_contract_verifier::{sourcify, sourcify::Error, SourcifyApiClient};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    sourcify_verifier_server::SourcifyVerifier, VerifyResponse, VerifyViaSourcifyRequest,
};
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
        request: Request<VerifyViaSourcifyRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request: VerifyViaSourcifyRequestWrapper = request.into_inner().into();
        let response = sourcify::api::verify(self.client.clone(), request.try_into()?).await;

        let result = match response {
            Ok(verification_success) => Ok(VerifyResponseWrapper::ok(verification_success.into())),
            Err(err) => match err {
                Error::Internal(err) => Err(Status::internal(err.to_string())),
                Error::Verification(err) => Ok(VerifyResponseWrapper::err(err)),
                Error::Validation(err) => Err(Status::invalid_argument(err)),
            },
        }?;

        metrics::count_verify_contract("solidity", &result.status, "sourcify");
        return Ok(Response::new(result.into_inner()));
    }
}
