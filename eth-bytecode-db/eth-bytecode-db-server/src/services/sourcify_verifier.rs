use crate::{
    proto::{sourcify_verifier_server, VerifyResponse, VerifySourcifyRequest},
    types::VerifyResponseWrapper,
};
use async_trait::async_trait;
use eth_bytecode_db::verification::{
    sourcify::{self, VerificationRequest},
    Client, Error,
};

pub struct SourcifyVerifierService {
    client: Client,
}

impl Default for SourcifyVerifierService {
    fn default() -> Self {
        todo!()
    }
}

#[async_trait]
impl sourcify_verifier_server::SourcifyVerifier for SourcifyVerifierService {
    async fn verify(
        &self,
        request: tonic::Request<VerifySourcifyRequest>,
    ) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
        let request = request.into_inner();

        let verification_request = VerificationRequest {
            address: request.address,
            chain: request.chain,
            chosen_contract: request.chosen_contract,
            source_files: request.files,
        };

        let result = sourcify::verify(self.client.clone(), verification_request).await;

        match result {
            Ok(source) => {
                let response = VerifyResponseWrapper::ok(source);
                Ok(tonic::Response::new(response.into()))
            }
            Err(Error::VerificationFailed { message }) => {
                let response = VerifyResponseWrapper::err(message);
                Ok(tonic::Response::new(response.into()))
            }
            Err(Error::InvalidArgument(message)) => Err(tonic::Status::invalid_argument(message)),
            Err(Error::Internal(message)) => Err(tonic::Status::internal(message.to_string())),
        }
    }
}
