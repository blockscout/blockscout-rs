use crate::logic::{AuthError, ConfigError, GithubError};
use sea_orm::DbErr;
use thiserror::Error;

mod deployment;
mod handlers;
mod instance;
mod instance_deployment;

pub use deployment::Deployment;
pub use handlers::*;
pub use instance::Instance;
pub use instance_deployment::InstanceDeployment;

#[derive(Error, Debug)]
pub enum DeployError {
    #[error("auth error: {0}")]
    Auth(#[from] AuthError),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("github error: {0}")]
    Github(#[from] GithubError),
    #[error("instance with name `{0}` already exists")]
    InstanceExists(String),
    #[error("instance with id `{0}` not found")]
    InstanceNotFound(String),
    #[error("deployment not found")]
    DeploymentNotFound,
    #[error("invalid action `{0}` for instance in state `{1}`")]
    InvalidStateTransition(String, String),
    #[error("db error: {0}")]
    Db(#[from] DbErr),
    #[error("github workflow error: {0}")]
    GithubWorkflow(anyhow::Error),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}
