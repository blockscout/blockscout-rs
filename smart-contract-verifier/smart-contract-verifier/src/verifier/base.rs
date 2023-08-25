use super::{
    bytecode::{BytecodePart, LocalBytecode},
    errors::VerificationError,
};
use crate::{DisplayBytes, MatchType};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalBytecodeParts {
    pub creation_tx_input_parts: Vec<BytecodePart>,
    pub deployed_bytecode_parts: Vec<BytecodePart>,
}

impl<T> From<LocalBytecode<T>> for LocalBytecodeParts {
    fn from(local_bytecode: LocalBytecode<T>) -> Self {
        LocalBytecodeParts {
            creation_tx_input_parts: local_bytecode.creation_tx_input_parts,
            deployed_bytecode_parts: local_bytecode.deployed_bytecode_parts,
        }
    }
}

/// The structure returned as a result when verification successes.
/// Contains data needed to be sent back as a verification response.
#[derive(Clone, Debug, PartialEq)]
pub struct VerificationSuccess {
    pub file_path: String,
    pub contract_name: String,
    pub abi: Option<serde_json::Value>,
    pub constructor_args: Option<DisplayBytes>,

    pub local_bytecode_parts: LocalBytecodeParts,
    pub match_type: MatchType,

    pub compilation_artifacts: serde_json::Value,
    pub creation_input_artifacts: serde_json::Value,
    pub deployed_bytecode_artifacts: serde_json::Value,
}

/// Combine different verifiers
pub trait Verifier: Send + Sync {
    /// Verification input (in most cases consists the output returned by compiler)
    type Input;

    /// Verifies provided input data
    fn verify(&self, input: &Self::Input) -> Result<VerificationSuccess, Vec<VerificationError>>;
}
