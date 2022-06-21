use crate::{
    compiler::{fetcher::Fetcher, version::CompilerVersion, Compilers, CompilersError},
    solidity::{VerificationSuccess, Verifier, VerifierInitializationError},
};
use ethers_solc::CompilerInput;
use std::fmt::{Debug, Display};
use thiserror::Error;

pub struct CompileAndVerifyInput<'a> {
    pub compiler_version: &'a CompilerVersion,
    pub compiler_input: &'a CompilerInput,
    pub creation_tx_input: &'a str,
    pub deployed_bytecode: &'a str,
}

#[derive(Error, Debug)]
pub(crate) enum CompileAndVerifyError {
    #[error("{0:#}")]
    VerifierInitialization(#[from] VerifierInitializationError),
    #[error("{0:#}")]
    Compilation(#[from] CompilersError),
    #[error("No contract could be verified with provided data")]
    NoMatchingContracts,
}

pub(crate) async fn compile_and_verify<T: Fetcher>(
    compilers: &Compilers<T>,
    input: &CompileAndVerifyInput<'_>,
) -> Result<VerificationSuccess, CompileAndVerifyError>
where
    <T as Fetcher>::Error: Debug + Display,
{
    let verifier = Verifier::new(input.creation_tx_input, input.deployed_bytecode)?;
    let compiler_output = compilers
        .compile(input.compiler_version, input.compiler_input)
        .await?;
    verifier
        .verify(compiler_output)
        .ok_or(CompileAndVerifyError::NoMatchingContracts)
}
