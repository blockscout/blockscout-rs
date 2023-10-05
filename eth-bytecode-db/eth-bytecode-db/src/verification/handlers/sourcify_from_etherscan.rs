use super::{
    super::{
        client::Client, errors::Error, smart_contract_verifier::VerifyFromEtherscanSourcifyRequest,
        types::Source,
    },
    process_verify_response, EthBytecodeDbAction, VerifierAllianceDbAction,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRequest {
    pub address: String,
    pub chain: String,
}

impl From<VerificationRequest> for VerifyFromEtherscanSourcifyRequest {
    fn from(request: VerificationRequest) -> Self {
        VerifyFromEtherscanSourcifyRequest {
            address: request.address,
            chain: request.chain,
        }
    }
}

pub async fn verify(mut client: Client, request: VerificationRequest) -> Result<Source, Error> {
    let request: VerifyFromEtherscanSourcifyRequest = request.into();
    let response = client
        .sourcify_client
        .verify_from_etherscan(request)
        .await
        .map_err(Error::from)?
        .into_inner();

    process_verify_response(
        response,
        EthBytecodeDbAction::IgnoreDb,
        VerifierAllianceDbAction::IgnoreDb,
    )
    .await
}
