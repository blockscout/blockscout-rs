use actix_web::{web::Json, Error};

use log::debug;

use super::types::{FlattenedSource, VerificationRequest, VerificationResponse};

type Request = VerificationRequest<FlattenedSource>;

pub async fn verify(params: Json<Request>) -> Result<Json<VerificationResponse>, Error> {
    debug!("verify contract with params {:?}", params);
    Ok(Json(VerificationResponse { verified: true }))
}
