use super::{
    api_client::SourcifyApiClient,
    types::{ApiRequest, ApiVerificationResponse, Error, Files, ResultItem, Success},
};
use crate::MatchType;
use anyhow::anyhow;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRequest {
    pub address: String,
    pub chain: String,
    pub files: BTreeMap<String, String>,
    pub chosen_contract: Option<usize>,
}

impl From<VerificationRequest> for ApiRequest {
    fn from(value: VerificationRequest) -> Self {
        Self {
            address: value.address,
            chain: value.chain,
            files: Files(value.files),
            chosen_contract: value.chosen_contract,
        }
    }
}

pub async fn verify(
    sourcify_client: Arc<SourcifyApiClient>,
    request: VerificationRequest,
) -> Result<Success, Error> {
    let params = request.into();
    let response = sourcify_client
        .verification_request(&params)
        .await
        .map_err(|err| {
            anyhow!(
                "error while making verification request to Sourcify: {}",
                err
            )
        })
        .map_err(Error::Internal)?;

    match response {
        ApiVerificationResponse::Verified { result } => {
            let api_files_response = sourcify_client
                .source_files_request(&params)
                .await
                .map_err(|err| {
                    anyhow!(
                        "error while making source files request to Sourcify: {}",
                        err
                    )
                })
                .map_err(Error::Internal)?;
            let files = Files::try_from((api_files_response, &params.chain, &params.address))
                .map_err(|err| anyhow!("error while parsing Sourcify files response: {}", err))
                .map_err(Error::Internal)?;
            let match_type = match_type_from_verification_result(result)?;
            let success = Success::try_from((files, match_type))
                .map_err(|err| Error::Validation(err.to_string()))?;

            if let Some(middleware) = sourcify_client.middleware() {
                middleware.call(&success).await;
            }

            Ok(success)
        }
        ApiVerificationResponse::Error { error } => Err(Error::Verification(error)),
        ApiVerificationResponse::ValidationErrors { message, errors } => {
            let error_message = format!("{message}: {errors:?}");
            Err(Error::Validation(error_message))
        }
    }
}

fn match_type_from_verification_result(result: Vec<ResultItem>) -> Result<MatchType, Error> {
    let item = result
        .get(0)
        .ok_or_else(|| {
            anyhow::anyhow!("invalid number of result items returned while verification succeeded")
        })
        .map_err(Error::Internal)?;
    match item.status.as_str() {
        "partial" => Ok(MatchType::Partial),
        "perfect" => Ok(MatchType::Full),
        _ => Err(Error::Internal(anyhow::anyhow!(
            "invalid match type status returned by the Sourcify instance"
        ))),
    }
}
