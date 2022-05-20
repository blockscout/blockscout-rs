use actix_web::{web::Json, Error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct VerificateResponse {
    verificated: bool,
}

pub async fn verificate() -> Result<Json<VerificateResponse>, Error> {
    Ok(Json(VerificateResponse { verificated: true }))
}
