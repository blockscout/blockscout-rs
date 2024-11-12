mod domain_name;
pub mod hash_name;
mod protocoler;

pub use domain_name::{DomainName, DomainNameOnProtocol};
pub use hash_name::domain_id;
pub use protocoler::*;

#[derive(thiserror::Error, Debug)]
pub enum ProtocolError {
    #[error("Network with id {0} not found")]
    NetworkNotFound(i64),
    #[error("name '{name}' is invalid: {reason}")]
    InvalidName { name: String, reason: String },
    #[error("protocol not found: {0}")]
    ProtocolNotFound(String),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}
