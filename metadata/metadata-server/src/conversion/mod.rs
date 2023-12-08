use thiserror::Error;

mod metadata;

pub use metadata::{get_metadata_request_from_inner, get_metadata_response_from_logic};

#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("invalid argument: {0}")]
    UserRequest(String),
    #[error("internal error: {0}")]
    #[allow(dead_code)]
    LogicOutput(String),
}
