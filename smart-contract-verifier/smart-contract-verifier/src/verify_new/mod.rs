mod cbor_auxdata;
mod compilation;
mod compiler_output;
mod evm_compilers;
mod solc_compiler;
mod verification;
mod verifier;

pub use evm_compilers::EvmCompilersPool;
pub use solc_compiler::{SolcCompiler, SolcInput};
pub use verification::{OnChainCode, RecompiledCode};
pub use verifier::{compile_and_verify, OnChainContract, VerificationResult, VerifyingContract};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("compiler not found: {0}")]
    CompilerNotFound(String),
    #[error("compilation error: {0:#?}")]
    Compilation(Vec<String>),
    #[error("{0:#?}")]
    Internal(#[from] anyhow::Error),
}
