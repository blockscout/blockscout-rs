use crate::{
    compiler::{self, Compilers},
    solidity::{VerificationSuccess, Verifier},
    VerificationResponse, VerificationResult,
};
use actix_web::error;
use ethers_solc::{
    artifacts::{BytecodeHash, SettingsMetadata},
    CompilerInput,
};
use semver::VersionReq;
use std::{fmt::Debug, path::PathBuf};
use thiserror::Error;
use tracing::instrument;

const BYTECODE_HASHES: [BytecodeHash; 3] =
    [BytecodeHash::Ipfs, BytecodeHash::None, BytecodeHash::Bzzr1];

#[derive(Debug)]
pub struct Input<'a> {
    pub compiler_version: compiler::Version,
    pub compiler_input: CompilerInput,
    pub creation_tx_input: &'a str,
    pub deployed_bytecode: &'a str,
}

#[derive(Error, Debug)]
enum CompileAndVerifyError {
    #[error("{0:#}")]
    Compilation(#[from] compiler::Error),
    #[error("No contract could be verified with provided data")]
    NoMatchingContracts,
}

pub(crate) async fn compile_and_verify_handler(
    compilers: &Compilers,
    mut input: Input<'_>,
    bruteforce_bytecode_hashes: bool,
) -> Result<VerificationResponse, actix_web::Error> {
    let verifier = Verifier::new(input.creation_tx_input, input.deployed_bytecode)
        .map_err(error::ErrorBadRequest)?;

    let bruteforce_metadata = settings_metadata(&input, bruteforce_bytecode_hashes);

    for metadata in bruteforce_metadata {
        input.compiler_input.settings.metadata = metadata;
        let result = compile_and_verify(compilers, &verifier, &input).await;
        if let Ok(verification_success) = result {
            let verification_result = VerificationResult::from((
                input.compiler_input,
                input.compiler_version,
                verification_success,
            ));
            return Ok(VerificationResponse::ok(verification_result));
        }
        if let Err(CompileAndVerifyError::Compilation(compiler::Error::Compilation(_))) = result {
            return Ok(VerificationResponse::err(result.unwrap_err()));
        }
        if let Err(CompileAndVerifyError::Compilation(compiler::Error::VersionNotFound(_))) = result
        {
            return Err(error::ErrorBadRequest(result.unwrap_err()));
        }
        if let Err(CompileAndVerifyError::Compilation(err)) = result {
            return Err(error::ErrorInternalServerError(err));
        }
    }
    // In case of any other error the execution will not get to this point
    Ok(VerificationResponse::err(
        CompileAndVerifyError::NoMatchingContracts,
    ))
}

#[instrument(skip(compilers, verifier, input), level = "debug")]
async fn compile_and_verify(
    compilers: &Compilers,
    verifier: &Verifier,
    input: &Input<'_>,
) -> Result<VerificationSuccess, CompileAndVerifyError> {
    let compiler_output = compilers
        .compile(&input.compiler_version, &input.compiler_input)
        .await?;
    let compiler_output_modified = {
        let mut compiler_input = input.compiler_input.clone();
        compiler_input
            .settings
            .libraries
            .libs
            .entry(PathBuf::from("SOME_TEXT_USED_AS_FILE_NAME"))
            .or_default()
            .insert(
                "SOME_TEXT_USED_AS_A_CONTRACT_NAME".into(),
                "0xcafecafecafecafecafecafecafecafecafecafe".into(),
            );
        compilers
            .compile(&input.compiler_version, &compiler_input)
            .await?
    };
    verifier
        .verify(compiler_output, compiler_output_modified)
        .map_err(|_errors| CompileAndVerifyError::NoMatchingContracts)
}

/// Iterates through possible bytecode if required and creates
/// a corresponding variants of settings metadata for each of them.
///
/// `bruteforce_bytecode_hashes` would be false for standard json input
/// as it contains the correct bytecode hash already. All other input
/// types do not specify it explicitly, thus, we have to iterate through
/// all possible options.
///
/// See "settings_metadata" (https://docs.soliditylang.org/en/v0.8.15/using-the-compiler.html?highlight=compiler%20input#input-description)
fn settings_metadata(
    input: &Input<'_>,
    bruteforce_bytecode_hashes: bool,
) -> Vec<Option<SettingsMetadata>> {
    if !bruteforce_bytecode_hashes {
        [input.compiler_input.settings.metadata.clone()].into()
    } else if VersionReq::parse("<0.6.0")
        .unwrap()
        .matches(input.compiler_version.version())
    {
        [None].into()
    } else {
        BYTECODE_HASHES
            .map(|hash| Some(SettingsMetadata::from(hash)))
            .into()
    }
}
