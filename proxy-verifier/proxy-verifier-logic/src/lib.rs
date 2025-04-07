mod address_details;
pub mod blockscout;
mod handlers;

pub use handlers::*;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    CompilationFailed(String),
    #[error("{0}")]
    InvalidContract(String),
    #[error("{0}")]
    Internal(String),
    #[error("{0}")]
    VerificationFailed(String),
}

impl Error {
    pub fn compilation_failed(message: impl Into<String>) -> Self {
        Self::CompilationFailed(message.into())
    }

    pub fn invalid_contract(message: impl Into<String>) -> Self {
        Self::InvalidContract(message.into())
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn not_contract() -> Self {
        Self::invalid_contract("Address is not a smart-contract")
    }

    pub fn no_runtime_code() -> Self {
        Self::invalid_contract("Smart-contract was self-destructed")
    }

    pub fn verification_failed(message: impl Into<String>) -> Self {
        Self::VerificationFailed(message.into())
    }

    pub fn is_compilation_failed_error(&self) -> bool {
        matches!(&self, Error::CompilationFailed(_))
    }

    pub fn is_invalid_contract_error(&self) -> bool {
        matches!(&self, Error::InvalidContract(_))
    }

    pub fn is_internal_error(&self) -> bool {
        matches!(&self, Error::Internal(_))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationSuccess {
    pub url: String,
    pub match_type: eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2::source::MatchType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationResponse {
    CompilationFailed(Error),
    InvalidContracts(Vec<Option<Error>>),
    Results(Vec<Result<VerificationSuccess, Error>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contract {
    pub chain_id: String,
    pub address: ethers_core::types::Address,
}
