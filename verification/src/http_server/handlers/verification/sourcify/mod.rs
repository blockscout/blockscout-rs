mod api;
mod metadata;
mod types;

pub use self::api::SourcifyApiClient;

use self::types::ApiRequest;
use actix_web::{error::Error, web, web::Json};

use super::VerificationResponse;
use crate::http_server::metrics;
use tracing::instrument;

#[instrument(skip(sourcify_client), level = "debug")]
pub async fn verify(
    sourcify_client: web::Data<SourcifyApiClient>,
    params: Json<ApiRequest>,
) -> Result<Json<VerificationResponse>, Error> {
    let response =
        api::verify_using_sourcify_client(sourcify_client.into_inner(), params.into_inner())
            .await?;
    metrics::count_verify_contract(&response.status, "sourcify");
    Ok(Json(response))
}
