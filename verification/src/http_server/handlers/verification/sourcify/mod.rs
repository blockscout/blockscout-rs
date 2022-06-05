mod api;
mod metadata;
mod types;

use self::api::verification_request;
use self::metadata::try_extract_metadata;
use self::types::{ApiRequest, ApiVerificationResponse};
use crate::Config;
use actix_web::web;
use actix_web::{error, error::Error, web::Json};

use super::VerificationResponse;

pub async fn verify(
    config: web::Data<Config>,
    params: Json<ApiRequest>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();
    let response = verification_request(&params, &config.sourcify.api_url)
        .await
        .map_err(error::ErrorInternalServerError)?;

    match response {
        ApiVerificationResponse::Verified { result: _ } => {
            let metadata = try_extract_metadata(&params, &config.sourcify.api_url)
                .await
                .map_err(error::ErrorInternalServerError)?;
            let response = VerificationResponse::try_from(metadata).unwrap();
            Ok(Json(response))
        }
        ApiVerificationResponse::Error { error } => Err(error::ErrorBadRequest(error)),
        ApiVerificationResponse::ValidationErrors { message, errors } => {
            let error_message = format!("{}: {:?}", message, errors);
            Err(error::ErrorBadRequest(error_message))
        }
    }
}
