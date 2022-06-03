mod metadata;
mod types;

use self::metadata::MetadataContent;
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
        ApiVerificationResponse::Verified { result: _ } => {
            // TODO: if metadata.json not found, make request to sourcify to get these files
            let metadata_content = params
                .files
                .get("metadata.json")
                .ok_or_else(|| error::ErrorBadRequest("metadata.json file not found"))?;
            let _response =
                VerificationResponse::try_from(MetadataContent(metadata_content)).unwrap();
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
