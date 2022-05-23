use actix_web::{Error, HttpResponse};

pub async fn status() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().into())
}
