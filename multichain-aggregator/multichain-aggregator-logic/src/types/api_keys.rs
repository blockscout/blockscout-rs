use super::ChainId;
use crate::error::{ParseError, ServiceError};
use entity::api_keys::Model;
use sea_orm::{entity::prelude::Uuid, DbErr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiKeyError {
    #[error("invalid token: {0}")]
    InvalidToken(String),
    #[error("db error: {0}")]
    Db(#[from] DbErr),
}

#[derive(Debug, Clone)]
pub struct ApiKey {
    pub key: Uuid,
    pub chain_id: ChainId,
}

impl TryFrom<(&str, &str)> for ApiKey {
    type Error = ServiceError;

    fn try_from(v: (&str, &str)) -> Result<Self, Self::Error> {
        Ok(Self {
            key: Uuid::parse_str(v.0).map_err(ParseError::from)?,
            chain_id: v.1.parse().map_err(ParseError::from)?,
        })
    }
}

impl From<Model> for ApiKey {
    fn from(v: Model) -> Self {
        Self {
            key: v.key,
            chain_id: v.chain_id,
        }
    }
}
