use super::{
    super::{
        client::Client, errors::Error, smart_contract_verifier::VerifySourcifyRequest,
        types::Source,
    },
    process_verify_response, EthBytecodeDbAction, VerifierAllianceDbAction,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRequest {
    pub address: String,
    pub chain: String,
    pub chosen_contract: Option<i32>,
    pub source_files: BTreeMap<String, String>,
}

impl From<VerificationRequest> for VerifySourcifyRequest {
    fn from(request: VerificationRequest) -> Self {
        VerifySourcifyRequest {
            address: request.address,
            chain: request.chain,
            files: request.source_files,
            chosen_contract: request.chosen_contract,
        }
    }
}

pub async fn verify(mut client: Client, request: VerificationRequest) -> Result<Source, Error> {
    let request: VerifySourcifyRequest = request.into();
    let response = client
        .sourcify_client
        .verify(request)
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
