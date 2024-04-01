use crate::logic::{ConfigError, GithubError};
use sea_orm::DbErr;
use thiserror::Error;

mod handlers;
mod instance;
mod types;

pub use handlers::*;
pub use instance::Instance;

#[derive(Error, Debug)]
pub enum DeployError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("github error: {0}")]
    Github(#[from] GithubError),
    #[error("instance with name `{0}` already exists")]
    InstanceExists(String),
    #[error("db error: {0}")]
    Db(#[from] DbErr),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}
