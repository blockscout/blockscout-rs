mod types;

use self::types::{ApiRequest, ApiVerificationResponse};
use crate::Config;
use actix_web::web;
use actix_web::{error, error::Error, web::Json};

use super::VerificationResponse;

async fn verification_request(
    config: &Config,
    params: &ApiRequest,
) -> Result<Json<VerificationResponse>, Error> {
    let resp = reqwest::Client::new()
        .post(&config.sourcify.api_url)
        .json(&params)
        .send()
        .await
        .map_err(error::ErrorInternalServerError)?;

    let response_body: ApiVerificationResponse =
        resp.json().await.map_err(error::ErrorInternalServerError)?;

    match response_body {
        ApiVerificationResponse::Verified { result } => {
            // TODO: parse metadata.json, return abi, constructor arguments, ...
            let _ = result;
            Ok(Json(VerificationResponse { verified: true }))
        }
        ApiVerificationResponse::Error { error } => Err(error::ErrorBadRequest(error)),
        ApiVerificationResponse::ValidationErrors { message, errors } => {
            let error_message = format!("{}: {:?}", message, errors);
            Err(error::ErrorBadRequest(error_message))
        }
    }
}

pub async fn verify(
    config: web::Data<Config>,
    params: Json<ApiRequest>,
) -> Result<Json<VerificationResponse>, Error> {
    verification_request(config.as_ref(), &params.into_inner()).await
}
