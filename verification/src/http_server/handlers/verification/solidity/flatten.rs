use actix_web::{web::Json, Error};

use log::debug;

use super::types::{FlattenedSource, VerificationRequest, VerificationResponse};

pub async fn verify(
    params: Json<VerificationRequest<FlattenedSource>>,
) -> Result<Json<VerificationResponse>, Error> {
    debug!("verify contract with params {:?}", params);
    Ok(Json(VerificationResponse { verified: true }))
}
