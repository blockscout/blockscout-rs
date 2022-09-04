use crate::verification_response::VerificationResponse;
use actix_web::{web, web::Json};
use serde::{Deserialize, Serialize};
use smart_contract_verifier::SourcifyApiClient;
use std::collections::BTreeMap;
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
    // let response =
    //     api::verify_using_sourcify_client(sourcify_client.into_inner(), params.into_inner())
    //         .await?;
    // metrics::count_verify_contract(&response.status, "sourcify");
    // Ok(Json(response))

    todo!()
}
