mod instance;
pub mod macros;
mod types;
mod user;
pub mod variables;

pub use instance::InstanceConfig;
pub use types::{ConfigValidationContext, ParsedVariable, ParsedVariableKey, UserVariable};
pub use user::UserConfig;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("failed to validate config: {0}")]
    Validation(String),

    #[error("missing config")]
    MissingConfig,

    #[error("missing variable: {0}")]
    MissingVariable(String),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}
