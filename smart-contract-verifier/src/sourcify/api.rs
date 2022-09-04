use super::api_client::SourcifyApiClient;
use std::{collections::BTreeMap, sync::Arc};

pub struct VerificationRequest {
    pub address: String,
    pub chain: String,
    pub files: BTreeMap<String, String>,
    pub chosen_contract: Option<usize>,
}

pub struct Error;

/// The public structure returned as a result when verification succeeds.
#[derive(Clone, Debug)]
pub struct Success {
    // pub compiler_input: CompilerInput,
    // pub compiler_version: Version,
    // pub file_path: String,
    // pub contract_name: String,
    // pub abi: ethabi::Contract,
    // pub constructor_args: Option<DisplayBytes>,
}

pub async fn verify(
    sourcify_client: Arc<SourcifyApiClient>,
    request: VerificationRequest,
) -> Result<Success, Error> {
    todo!()
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
