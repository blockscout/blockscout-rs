mod generated;
pub mod macros;
mod types;
mod validated;
pub mod variables;

pub use generated::GeneratedInstanceConfig;
pub use types::{ParsedVariable, ParsedVariableKey, UserVariable};
pub use validated::ValidatedInstanceConfig;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to validate config: {0}")]
    Validation(anyhow::Error),

    #[error("internal error: {0}")]
    Internal(anyhow::Error),
}
