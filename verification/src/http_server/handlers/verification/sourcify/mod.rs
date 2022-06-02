mod types;

use self::types::{ApiRequest, ApiVerificationResponse};
use crate::{Config, VerificationResult};
use actix_web::web;
use actix_web::{error, error::Error, web::Json};
use std::collections::HashMap;

use super::{VerificationResponse, VerificationStatus};

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
            let verification_result = VerificationResult {
                contract_name: "".into(),
                compiler_version: "".into(),
                evm_version: "".into(),
                constructor_arguments: None,
                contract_libraries: None,
                abi: "".into(),
                sources: HashMap::new(),
            };
            Ok(Json(VerificationResponse::ok(verification_result)))
        }
        ApiVerificationResponse::Error { error } => Ok(Json(VerificationResponse::err(
            VerificationStatus::UnknowError,
            Some(error),
        ))),
        ApiVerificationResponse::ValidationErrors { message, errors } => {
            let error_message = format!("{}: {:?}", message, errors);
            Ok(Json(VerificationResponse::err(
                VerificationStatus::UnknowError,
                Some(error_message),
            )))
        }
    }
}

pub async fn verify(
    config: web::Data<Config>,
    params: Json<ApiRequest>,
) -> Result<Json<VerificationResponse>, Error> {
    verification_request(config.as_ref(), &params.into_inner()).await
}
