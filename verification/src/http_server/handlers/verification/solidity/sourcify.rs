use actix_web::{error, error::Error, web::Json};
use serde::Deserialize;

use crate::Configuration;

use super::types::{SourcifyRequest, VerificationResponse};
use actix_web::web;

// https://docs.sourcify.dev/docs/api/server/v1/verify/
#[derive(Deserialize)]
#[serde(untagged)]
enum SourifyApiResponse {
    Verified {
        result: Vec<SourcifyResultItem>,
    },
    Error {
        error: String,
    },
    ValidationErrors {
        message: String,
        errors: Vec<ValidationError>,
    },
}

#[derive(Deserialize)]
struct SourcifyResultItem {
    address: String,
    status: String,
    storageTimestamp: Option<String>,
}

#[derive(Deserialize)]
struct ValidationError {
    field: String,
    message: String,
}

async fn sourcify_verification_request(
    config: &Configuration,
    params: &SourcifyRequest,
) -> Result<Json<VerificationResponse>, Error> {
    let resp = reqwest::Client::new()
        .post(&config.urls.sourcify_api)
        .json(&params)
        .send()
        .await
        .map_err(error::ErrorInternalServerError)?;

    let response_body: SourifyApiResponse =
        resp.json().await.map_err(error::ErrorInternalServerError)?;

    match response_body {
        SourifyApiResponse::Verified { result } => {
            // TODO: return abi, constructor arguments, ...
            let _ = result;
            Ok(Json(VerificationResponse { verified: true }))
        }
        SourifyApiResponse::Error { error } => Err(error::ErrorUnprocessableEntity(error)),
        SourifyApiResponse::ValidationErrors { message, errors } => {
            Err(error::ErrorUnprocessableEntity(message))
        }
    }
}

pub async fn verify(
    config: web::Data<Configuration>,
    params: Json<SourcifyRequest>,
) -> Result<Json<VerificationResponse>, Error> {
    sourcify_verification_request(config.as_ref(), &params.0).await
}
