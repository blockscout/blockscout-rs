use super::{
    super::{
        client::Client, errors::Error, smart_contract_verifier::VerifySourcifyRequest,
        types::Source, VerificationMetadata,
    },
    process_verify_response, EthBytecodeDbAction, VerifierAllianceDbAction,
};
use serde::{Deserialize, Serialize};
use smart_contract_verifier_proto::http_client::sourcify_verifier_client;
use std::{collections::BTreeMap, str::FromStr};

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

impl<'a> From<&'a VerificationRequest> for VerificationMetadata {
    fn from(value: &'a VerificationRequest) -> Self {
        Self {
            chain_id: i64::from_str(&value.chain).ok(),
            contract_address: blockscout_display_bytes::Bytes::from_str(&value.address)
                .ok()
                .map(|v| v.0),
            ..Default::default()
        }
    }
}

pub async fn verify(client: Client, request: VerificationRequest) -> Result<Source, Error> {
    let verification_metadata = VerificationMetadata::from(&request);
    let request: VerifySourcifyRequest = request.into();

    let response = sourcify_verifier_client::verify(&client.verifier_http_client, request).await?;

    process_verify_response(
        response,
        EthBytecodeDbAction::SaveOnlyAbiData {
            db_client: client.db_client.as_ref(),
            verification_metadata: Some(verification_metadata),
        },
        VerifierAllianceDbAction::IgnoreDb,
    )
    .await
}
