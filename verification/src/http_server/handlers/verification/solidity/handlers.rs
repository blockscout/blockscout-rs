use crate::{
    compiler::{CompilerVersion, Compilers, CompilersError, Fetcher},
    solidity::{VerificationSuccess, Verifier, VerifierInitializationError},
    VerificationResponse, VerificationResult,
};
use actix_web::error;
use ethers_solc::{
    artifacts::{BytecodeHash, SettingsMetadata},
    CompilerInput,
};
use std::{
    fmt::{Debug, Display},
    str::FromStr,
};
use thiserror::Error;

const BYTECODE_HASHES: [BytecodeHash; 3] =
    [BytecodeHash::Ipfs, BytecodeHash::None, BytecodeHash::Bzzr1];

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
    mut input: CompileAndVerifyInput<'_>,
    bruteforce_bytecode_hashes: bool,
) -> Result<VerificationResponse, actix_web::Error>
where
    <T as Fetcher>::Error: Debug + Display,
{
    let bruteforce_metadata = if !bruteforce_bytecode_hashes {
        Vec::from([input.compiler_input.settings.metadata.clone()])
    } else if pre_v0_6_0(&input.compiler_version) {
        Vec::from([None])
    } else {
        Vec::from(BYTECODE_HASHES.map(|hash| Some(SettingsMetadata::from(hash))))
    };

    for metadata in bruteforce_metadata {
        input.compiler_input.settings.metadata = metadata;
        match compile_and_verify(compilers, &input).await {
            Ok(verification_success) => {
                let verification_result = VerificationResult::from((
                    input.compiler_input,
                    input.compiler_version,
                    verification_success,
                ));
                return Ok(VerificationResponse::ok(verification_result));
            }
            Err(CompileAndVerifyError::VerifierInitialization(err)) => {
                return Err(error::ErrorBadRequest(err))
            }
            Err(CompileAndVerifyError::Compilation(CompilersError::Fetch(err))) => {
                return Err(error::ErrorInternalServerError(err))
            }
            Err(CompileAndVerifyError::Compilation(CompilersError::Internal(err))) => {
                return Err(error::ErrorInternalServerError(err))
            }
            err @ Err(CompileAndVerifyError::Compilation(CompilersError::Compilation(_))) => {
                return Ok(VerificationResponse::err(err.expect_err("Matched to Err")))
            }
            // Try other bytecode hashes if there is no matching contracts
            Err(CompileAndVerifyError::NoMatchingContracts) => {}
        }
    }
    // In case of any other error the execution will not get to this point
    Ok(VerificationResponse::err(
        CompileAndVerifyError::NoMatchingContracts,
    ))
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

fn pre_v0_6_0(compiler_version: &CompilerVersion) -> bool {
    let last_non_0_6_0_version =
        CompilerVersion::from_str("v0.5.17+commit.d19bba13").expect("Valid compiler version");
    compiler_version <= &last_non_0_6_0_version
}
