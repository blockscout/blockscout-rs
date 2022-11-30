use crate::DisplayBytes;
use mismatch::Mismatch;
use std::fmt::{Display, Formatter};
use thiserror::Error;

/// Errors that may occur during deployed bytecode and
/// creation transaction input parsing.
#[derive(Error, Clone, Debug, PartialEq, Eq)]
pub enum BytecodeInitError {
    #[error("creation transaction input is not valid: {0}")]
    InvalidCreationTxInput(String),
    #[error("deployed bytecode is not valid: {0}")]
    InvalidDeployedBytecode(String),
    #[error("bytecode is empty")]
    Empty,
}

/// Enumerates errors that may occur during a single contract verification.
#[derive(Error, Clone, Debug, PartialEq, Eq)]
pub enum VerificationErrorKind {
    #[error("internal error: {0}")]
    InternalError(String),
    #[error("library missed")]
    LibraryMissed,
    #[error("contract is abstract")]
    AbstractContract,
    #[error("bytecode length is less than expected: {part}; bytecodes: {raw}")]
    BytecodeLengthMismatch {
        part: Mismatch<usize>,
        raw: Mismatch<DisplayBytes>,
    },
    #[error("bytecode does not match compilation output: {part}; bytecodes: {raw}")]
    BytecodeMismatch {
        part: Mismatch<DisplayBytes>,
        raw: Mismatch<DisplayBytes>,
    },
    #[error("cannot parse metadata")]
    MetadataParse(String),
    #[error("compiler versions included into metadata hash does not match: {0}")]
    CompilerVersionMismatch(Mismatch<semver::Version>),
    #[error("invalid constructor arguments: {0}")]
    InvalidConstructorArguments(DisplayBytes),
}

/// Error obtained as a result of a single contract verification.
/// Is used to return more details about verification process to the caller.
///
/// Returns the contract for which error occurred and the error itself.
#[derive(Error, Clone, Debug, PartialEq, Eq)]
pub struct VerificationError {
    pub file_path: String,
    pub contract_name: Option<String>,
    #[source]
    pub kind: VerificationErrorKind,
}

impl Display for VerificationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.contract_name {
            None => {
                write!(f, "{} - {}", self.file_path, self.kind)
            }
            Some(name) => {
                write!(f, "{}:{} - {}", self.file_path, name, self.kind)
            }
        }
    }
}

impl VerificationError {
    pub const fn new(file_path: String, kind: VerificationErrorKind) -> Self {
        Self {
            file_path,
            contract_name: None,
            kind,
        }
    }

    pub const fn with_contract(
        file_path: String,
        contract_name: String,
        kind: VerificationErrorKind,
    ) -> Self {
        Self {
            file_path,
            contract_name: Some(contract_name),
            kind,
        }
    }
}
