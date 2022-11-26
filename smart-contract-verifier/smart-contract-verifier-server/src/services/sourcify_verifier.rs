use crate::settings::SourcifySettings;
use smart_contract_verifier::SourcifyApiClient;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    sourcify_verifier_server::SourcifyVerifier, VerifyResponse, VerifyViaSourcifyRequest,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct SourcifyVerifierService {
    _client: Arc<SourcifyApiClient>,
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
            _client: Arc::new(client),
        })
    }
}

#[async_trait::async_trait]
impl SourcifyVerifier for SourcifyVerifierService {
    async fn verify(
        &self,
        _request: Request<VerifyViaSourcifyRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        todo!()
    }
}
