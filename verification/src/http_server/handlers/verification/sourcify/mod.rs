mod types;

use self::types::{SourcifyRequest, SourcifyVerifyResponse};
use crate::{http_server::handlers::VerificationResponse, Config};
use actix_web::web;
use actix_web::{error, error::Error, web::Json};

async fn sourcify_verification_request(
    config: &Config,
    params: &SourcifyRequest,
) -> Result<Json<VerificationResponse>, Error> {
    let resp = reqwest::Client::new()
        .post(&config.sourcify.api_url)
        .json(&params)
        .send()
        .await
        .map_err(error::ErrorInternalServerError)?;

    let response_body: SourcifyVerifyResponse =
        resp.json().await.map_err(error::ErrorInternalServerError)?;

    match response_body {
        SourcifyVerifyResponse::Verified { result } => {
            // TODO: parse metadata.json, return abi, constructor arguments, ...
            let _ = result;
            Ok(Json(VerificationResponse { verified: true }))
        }
        SourcifyVerifyResponse::Error { error } => Err(error::ErrorBadRequest(error)),
        SourcifyVerifyResponse::ValidationErrors { message, errors } => {
            let error_message = format!("{}: {:?}", message, errors);
            Err(error::ErrorBadRequest(error_message))
        }
    }
}

pub async fn verify(
    config: web::Data<Config>,
    params: Json<SourcifyRequest>,
) -> Result<Json<VerificationResponse>, Error> {
    sourcify_verification_request(config.as_ref(), &params.into_inner()).await
}
