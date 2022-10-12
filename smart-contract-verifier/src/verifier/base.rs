use super::errors::VerificationError;
use crate::DisplayBytes;

/// The structure returned as a result when verification successes.
/// Contains data needed to be sent back as a verification response.
#[derive(Clone, Debug, PartialEq)]
pub struct VerificationSuccess {
    pub file_path: String,
    pub contract_name: String,
    pub abi: Option<ethabi::Contract>,
    pub constructor_args: Option<DisplayBytes>,
}

/// Combine different verifiers
pub trait Verifier {
    /// Verification input (in most cases consists the output returned by compiler)
    type Input;

    /// Verifies provided input data
    fn verify(&self, input: Self::Input) -> Result<VerificationSuccess, Vec<VerificationError>>;
}
