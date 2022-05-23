pub mod handlers;
pub mod routes;
pub mod server;

use actix_web::{web::Json, Error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct HealthResponse {
    pub status: String,
}

pub async fn status() -> Result<Json<HealthResponse>, Error> {
    Ok(Json(HealthResponse {
        status: "ok".into(),
    }))
}
