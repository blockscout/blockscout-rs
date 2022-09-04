use std::collections::BTreeMap;
use smart_contract_verifier::SourcifyApiClient;
use actix_web::web;
use actix_web::web::Json;
use serde::{Serialize, Deserialize};
use super::verification_response::VerificationResponse;
use tracing::instrument;

// This struct is used as input for our endpoint and as
// input for sourcify endpoint at the same time
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequest {
    pub address: String,
    pub chain: String,
    pub files: Files,
    pub chosen_contract: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Files(pub BTreeMap<String, String>);

#[instrument(skip(sourcify_client, params), level = "debug")]
pub async fn verify(
    sourcify_client: web::Data<SourcifyApiClient>,
    params: Json<ApiRequest>,
) -> Result<Json<VerificationResponse>, actix_web::Error> {
    todo!()
    // let response =
    //     api::verify_using_sourcify_client(sourcify_client.into_inner(), params.into_inner())
    //         .await?;
    // metrics::count_verify_contract(&response.status, "sourcify");
    // Ok(Json(response))
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
