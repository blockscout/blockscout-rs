use crate::{
    metrics,
    settings::SourcifySettings,
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
    pub fn new(settings: SourcifySettings) -> anyhow::Result<Self> {
        let client = SourcifyApiClient::new(
            settings.api_url,
            settings.request_timeout,
            settings.verification_attempts,
        )
        .expect("failed to build sourcify client");
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
