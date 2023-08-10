use crate::{metrics, verification_response::VerificationResponse};
use actix_web::{error, web, web::Json};
use serde::{Deserialize, Serialize};
use smart_contract_verifier::{
    sourcify::{api, Error},
    SourcifyApiClient,
};
use std::collections::BTreeMap;
use tracing::instrument;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequest {
    pub address: String,
    pub chain: String,
    pub files: BTreeMap<String, String>,
    pub chosen_contract: Option<usize>,
}

impl From<ApiRequest> for api::VerificationRequest {
    fn from(value: ApiRequest) -> Self {
        Self {
            address: value.address,
            chain: value.chain,
            files: value.files,
            chosen_contract: value.chosen_contract,
        }
    }
}

#[instrument(skip(sourcify_client, params), level = "debug")]
pub async fn verify(
    sourcify_client: web::Data<SourcifyApiClient>,
    params: Json<ApiRequest>,
) -> Result<Json<VerificationResponse>, actix_web::Error> {
    let request = params.into_inner().try_into()?;

    let response = api::verify(sourcify_client.into_inner(), request).await;
    let response = match response {
        Ok(success) => Ok(VerificationResponse::ok(success.into())),
        Err(err) => match err {
            Error::Internal(err) => Err(error::ErrorInternalServerError(err)),
            Error::Verification(err) => Ok(VerificationResponse::err(err)),
            Error::Validation(err) => Err(error::ErrorBadRequest(err)),
            Error::BadRequest(err) => Err(error::ErrorBadRequest(err)),
        },
    }?;
    metrics::count_verify_contract("solidity", &response.status, "sourcify");
    Ok(Json(response))
}
