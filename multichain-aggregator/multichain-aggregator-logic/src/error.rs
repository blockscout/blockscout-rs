use crate::types::api_keys::ApiKeyError;
use alloy_primitives::hex::FromHexError;
use sea_orm::{sqlx::types::uuid, DbErr};
use std::num::ParseIntError;
use thiserror::Error;
use tonic::Code;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("api key error: {0}")]
    ApiKey(#[from] ApiKeyError),
    #[error(transparent)]
    Convert(#[from] ParseError),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("external api error: {0}")]
    ExternalApi(#[from] api_client_framework::Error),
    #[error("db error: {0}")]
    Db(#[from] DbErr),
    #[error("not found: {0}")]
    NotFound(String),
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("parse error: invalid integer")]
    ParseInt(#[from] ParseIntError),
    #[error("parse error: invalid hex")]
    ParseHex(#[from] FromHexError),
    #[error("parse error: invalid uuid")]
    ParseUuid(#[from] uuid::Error),
    #[error("parse error: invalid slice")]
    TryFromSlice(#[from] core::array::TryFromSliceError),
    #[error("parse error: invalid url")]
    ParseUrl(#[from] url::ParseError),
    #[error("parse error: invalid json")]
    Json(#[from] serde_json::Error),
    #[error("parse error: {0}")]
    Custom(String),
}

impl From<ServiceError> for tonic::Status {
    fn from(err: ServiceError) -> Self {
        let code = match &err {
            ServiceError::ApiKey(err) => map_api_key_code(err),
            ServiceError::Convert(_) => Code::InvalidArgument,
            ServiceError::Internal(_) => Code::Internal,
            ServiceError::NotFound(_) => Code::NotFound,
            ServiceError::Db(_) => Code::Internal,
            ServiceError::ExternalApi(_) => Code::Internal,
        };
        tonic::Status::new(code, err.to_string())
    }
}

fn map_api_key_code(err: &ApiKeyError) -> Code {
    match err {
        ApiKeyError::InvalidToken(_) => Code::PermissionDenied,
        ApiKeyError::Db(_) => Code::Internal,
    }
}
