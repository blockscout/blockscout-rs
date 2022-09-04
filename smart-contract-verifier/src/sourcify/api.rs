use super::api_client::SourcifyApiClient;
use super::types::{ApiRequest, ApiVerificationResponse, Files, Success, Error};
use anyhow::anyhow;
use std::collections::BTreeMap;
use std::sync::Arc;

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
        ApiVerificationResponse::Verified { result: _ } => {
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
            let files = Files::try_from(api_files_response)
                .map_err(|err| anyhow!("error while parsing Sourcify files response: {}", err))
                .map_err(Error::Internal)?;
            let success = Success::try_from(files).map_err(|err| Error::Validation(err.to_string()))?;
            Ok(success)
        }
        ApiVerificationResponse::Error { error } => Err(Error::Verification(error)),
        ApiVerificationResponse::ValidationErrors { message, errors } => {
            let error_message = format!("{}: {:?}", message, errors);
            Err(Error::Validation(error_message))
        }
    }
}

// pub async fn verify(
//     sourcify_client: Arc<impl SourcifyApi>,
//     params: ApiRequest,
// ) -> Result<VerificationResponse, Error> {
//     let response = sourcify_client
//         .verification_request(&params)
//         .await
//         .map_err(error::ErrorInternalServerError)?;
//
//     match response {
//         ApiVerificationResponse::Verified { result: _ } => {
//             let api_files_response = sourcify_client
//                 .source_files_request(&params)
//                 .await
//                 .map_err(error::ErrorInternalServerError)?;
//             let files =
//                 Files::try_from(api_files_response).map_err(error::ErrorInternalServerError)?;
//             let result = VerificationResult::try_from(files).map_err(error::ErrorBadRequest)?;
//             Ok(VerificationResponse::ok(result))
//         }
//         ApiVerificationResponse::Error { error } => Ok(VerificationResponse::err(error)),
//         ApiVerificationResponse::ValidationErrors { message, errors } => {
//             let error_message = format!("{}: {:?}", message, errors);
//             Err(error::ErrorBadRequest(error_message))
//         }
//     }
// }
