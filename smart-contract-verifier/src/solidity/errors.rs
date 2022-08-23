use crate::{types::Mismatch, DisplayBytes};
use thiserror::Error;

/// Errors that may occur during deployed bytecode and
/// creation transaction input parsing.
#[derive(Error, Clone, Debug, PartialEq, Eq)]
pub enum BytecodeInitializationError {
    #[error("creation transaction input is not a valid hex string: {0}")]
    InvalidCreationTxInput(String),
    #[error("deployed bytecode is not a valid hex string: {0}")]
    InvalidDeployedBytecode(String),
    #[error("deployed bytecode does not correspond to creation tx input: {0}")]
    BytecodeMismatch(Mismatch<DisplayBytes>),
}

/// Enumerates errors that may occur during a single contract verification.
#[derive(Error, Clone, Debug, PartialEq, Eq)]
pub enum VerificationErrorKind {}

/// Error obtained as a result of a single contract verification.
/// Is used to return more details about verification process to the caller.
///
/// Returns the contract for which error occurred and the error itself.
#[derive(Error, Clone, Debug, PartialEq, Eq)]
#[error("{file_path}:{contract_name}: {kind}")]
pub struct VerificationError {
    pub file_path: String,
    pub contract_name: String,
    #[source]
    pub kind: VerificationErrorKind,
}
