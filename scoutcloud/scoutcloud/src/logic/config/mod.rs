mod generated;
pub mod macros;
mod types;
mod validated;
pub mod variables;

pub use generated::GeneratedInstanceConfig;
pub use types::{ConfigValidationContext, ParsedVariable, ParsedVariableKey, UserVariable};
pub use validated::ValidatedInstanceConfig;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("failed to validate config: {0}")]
    Validation(String),

    #[error("internal error: {0}")]
    Internal(anyhow::Error),
}
