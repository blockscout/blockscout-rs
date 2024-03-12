use crate::DisplayBytes;
use mismatch::Mismatch;
use std::fmt::{Display, Formatter};
use thiserror::Error;

/// Enumerates errors that may occur during a single contract verification.
#[derive(Error, Clone, Debug, PartialEq, Eq)]
pub enum VerificationErrorKind {
    #[error("internal error: {0}")]
    InternalError(String),
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
    #[error("neither creation nor runtime code do not have a match")]
    CodeMismatch,
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
    pub contract_name: String,
    #[source]
    pub kind: VerificationErrorKind,
}

impl Display for VerificationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{} - {}",
            self.file_path, self.contract_name, self.kind
        )
    }
}

impl VerificationError {
    pub const fn new(
        file_path: String,
        contract_name: String,
        kind: VerificationErrorKind,
    ) -> Self {
        Self {
            file_path,
            contract_name,
            kind,
        }
    }
}
