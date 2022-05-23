use actix_web::{HttpResponse, Responder};

pub async fn status() -> impl Responder {
    HttpResponse::Ok().finish()
}
