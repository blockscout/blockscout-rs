mod api;
mod metadata;
mod types;

use self::api::SoucifyApiClient;
use self::types::{ApiRequest, ApiVerificationResponse, Files};
use crate::Config;
use actix_web::web;
use actix_web::{error, error::Error, web::Json};

use super::VerificationResponse;

pub async fn verify(
    config: web::Data<Config>,
    params: Json<ApiRequest>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();
    let sourcify_client = SoucifyApiClient::new(&config.sourcify.api_url);
    let response = sourcify_client
        .verification(&params)
        .await
        .map_err(error::ErrorInternalServerError)?;

    match response {
        ApiVerificationResponse::Verified { result: api_result } => {
            let files = {
                let contract_was_already_verified = api_result
                    .first()
                    .ok_or_else(|| error::ErrorInternalServerError("sourcify empty response"))?
                    .storage_timestamp
                    .is_some();
                if contract_was_already_verified {
                    Files::try_from(
                        sourcify_client
                            .source_files(&params)
                            .await
                            .map_err(error::ErrorInternalServerError)?,
                    )
                    .map_err(error::ErrorInternalServerError)?
                } else {
                    params.files
                }
            };
            let response = VerificationResponse::try_from(files).map_err(error::ErrorBadRequest)?;
            Ok(Json(response))
        }
        ApiVerificationResponse::Error { error } => Err(error::ErrorBadRequest(error)),
        ApiVerificationResponse::ValidationErrors { message, errors } => {
            let error_message = format!("{}: {:?}", message, errors);
            Err(error::ErrorBadRequest(error_message))
        }
    }
}
