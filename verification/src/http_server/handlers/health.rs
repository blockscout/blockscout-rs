use actix_web::{web::Json, Error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct HealthResponse {
    pub status: String,
}

pub async fn get_health() -> Result<Json<HealthResponse>, Error> {
    Ok(Json(HealthResponse {
        status: "ok".into(),
    }))
}
