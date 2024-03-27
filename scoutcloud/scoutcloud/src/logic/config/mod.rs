pub mod generated;
pub mod validated;
pub mod variables;

pub use crate::logic::config::variables::{ParsedVariable, ParsedVariableKey, UserVariable};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to validate config: {0}")]
    Validation(anyhow::Error),

    #[error("internal error: {0}")]
    Internal(anyhow::Error),
}
