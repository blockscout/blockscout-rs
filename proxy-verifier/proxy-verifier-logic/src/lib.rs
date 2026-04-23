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
    #[error("Your code is valid and verifies this contract (partial match). However, this contract is already partially verified and can only be reverified with a full (exact) match\n[{0}].")]
    AlreadyFullyVerifiedContract(String),
    #[error("Your code is valid and verifies this contract (partial match). However, this contract is already partially verified with identical functionality\n[{0}].")]
    AlreadyPartiallyVerifiedContract(String),
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

    pub fn already_fully_verified_contract(url: &url::Url) -> Self {
        Self::AlreadyFullyVerifiedContract(url.to_string())
    }

    pub fn already_partially_verified_contract(url: &url::Url) -> Self {
        Self::AlreadyPartiallyVerifiedContract(url.to_string())
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
