use super::{
    super::{
        client::Client,
        errors::Error,
        smart_contract_verifier::VerifyViaSourcifyRequest,
        types::{Source, SourceType},
    },
    process_verify_response, ProcessResponseAction,
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

impl From<VerificationRequest> for VerifyViaSourcifyRequest {
    fn from(request: VerificationRequest) -> Self {
        VerifyViaSourcifyRequest {
            address: request.address,
            chain: request.chain,
            files: request.source_files,
            chosen_contract: request.chosen_contract,
        }
    }
}

pub async fn verify(mut client: Client, request: VerificationRequest) -> Result<Source, Error> {
    let request: VerifyViaSourcifyRequest = request.into();
    let response = client
        .sourcify_client
        .verify(request)
        .await
        .map_err(Error::from)?
        .into_inner();

    let source_type_fn = |_file_name: &str| Ok(SourceType::Solidity);

    process_verify_response(
        &client.db_client,
        response,
        source_type_fn,
        ProcessResponseAction::IgnoreDb,
    )
    .await
}
