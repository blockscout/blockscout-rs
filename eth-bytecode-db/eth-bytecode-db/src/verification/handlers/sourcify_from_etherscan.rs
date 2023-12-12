use super::{
    super::{
        client::Client, errors::Error, smart_contract_verifier::VerifyFromEtherscanSourcifyRequest,
        types::Source, VerificationMetadata,
    },
    process_verify_response, EthBytecodeDbAction, VerifierAllianceDbAction,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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

pub async fn verify(mut client: Client, request: VerificationRequest) -> Result<Source, Error> {
    let verification_metadata = VerificationMetadata::from(&request);
    let request: VerifyFromEtherscanSourcifyRequest = request.into();
    let response = client
        .sourcify_client
        .verify_from_etherscan(request)
        .await
        .map_err(Error::from)?
        .into_inner();

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
