use crate::{
    compiler::{CompilerVersion, Compilers, CompilersError, Fetcher},
    solidity::{VerificationSuccess, Verifier, VerifierInitializationError},
    VerificationResponse, VerificationResult,
};
use actix_web::error;
use ethers_solc::CompilerInput;
use std::fmt::{Debug, Display};
use thiserror::Error;

pub struct CompileAndVerifyInput<'a> {
    pub compiler_version: CompilerVersion,
    pub compiler_input: CompilerInput,
    pub creation_tx_input: &'a str,
    pub deployed_bytecode: &'a str,
}

#[derive(Error, Debug)]
enum CompileAndVerifyError {
    #[error("{0:#}")]
    VerifierInitialization(#[from] VerifierInitializationError),
    #[error("{0:#}")]
    Compilation(#[from] CompilersError),
    #[error("No contract could be verified with provided data")]
    NoMatchingContracts,
}

pub(crate) async fn compile_and_verify_handler<T: Fetcher>(
    compilers: &Compilers<T>,
    input: CompileAndVerifyInput<'_>,
) -> Result<VerificationResponse, actix_web::Error>
where
    <T as Fetcher>::Error: Debug + Display,
{
    match compile_and_verify(compilers, &input).await {
        Ok(verification_success) => {
            let verification_result = VerificationResult::from((
                input.compiler_input,
                input.compiler_version,
                verification_success,
            ));
            Ok(VerificationResponse::ok(verification_result))
        }
        Err(CompileAndVerifyError::VerifierInitialization(err)) => Err(error::ErrorBadRequest(err)),
        Err(CompileAndVerifyError::Compilation(CompilersError::Fetch(err))) => {
            Err(error::ErrorInternalServerError(err))
        }
        Err(CompileAndVerifyError::Compilation(CompilersError::Internal(err))) => {
            Err(error::ErrorInternalServerError(err))
        }
        Err(err) => Ok(VerificationResponse::err(err)),
    }
}

async fn compile_and_verify<T: Fetcher>(
    compilers: &Compilers<T>,
    input: &CompileAndVerifyInput<'_>,
) -> Result<VerificationSuccess, CompileAndVerifyError>
where
    <T as Fetcher>::Error: Debug + Display,
{
    let verifier = Verifier::new(input.creation_tx_input, input.deployed_bytecode)?;
    let compiler_output = compilers
        .compile(&input.compiler_version, &input.compiler_input)
        .await?;
    verifier
        .verify(compiler_output)
        .ok_or(CompileAndVerifyError::NoMatchingContracts)
}
