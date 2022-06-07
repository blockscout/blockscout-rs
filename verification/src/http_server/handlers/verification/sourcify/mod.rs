mod api;
mod metadata;
mod types;

use self::api::{verify_using_sourcify_client, SoucifyApiClient};
use self::types::ApiRequest;
use crate::Config;
use actix_web::web;
use actix_web::{error::Error, web::Json};

use super::VerificationResponse;

pub async fn verify(
    config: web::Data<Config>,
    params: Json<ApiRequest>,
) -> Result<Json<VerificationResponse>, Error> {
    let sourcify_client = SoucifyApiClient::new(&config.sourcify.api_url);
    let response = verify_using_sourcify_client(sourcify_client, params.into_inner()).await?;
    Ok(Json(response))
}
