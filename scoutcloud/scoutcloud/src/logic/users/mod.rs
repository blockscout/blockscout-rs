mod handlers;
mod register_promo;
pub mod user_actions;
mod user_token;

pub use handlers::*;
pub use user_token::*;

use register_promo::PromoError;
use sea_orm::DbErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("no token provided, use x-api-key header to provide token")]
    NoToken,
    #[error("token not found")]
    TokenNotFound,
    #[error("requested resource not found")]
    NotFound,
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("invalid usage of promo: {0}")]
    Promo(#[from] PromoError),
    #[error("db error: {0}")]
    Db(#[from] DbErr),
}
