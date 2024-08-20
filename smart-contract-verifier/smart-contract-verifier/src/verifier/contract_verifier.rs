use super::{
    all_metadata_extracting_verifier, base,
    base::LocalBytecodeParts,
    bytecode::{CreationTxInput, CreationTxInputWithoutConstructorArgs, DeployedBytecode},
    errors::{BytecodeInitError, VerificationError, VerificationErrorKind},
    lossless_compiler_output,
};
use crate::{
    compiler::{self, CompilerInput, Compilers, EvmCompiler},
    DisplayBytes, MatchType,
};
use anyhow::{anyhow, Context};
use bytes::Bytes;
use ethers_solc::CompilerOutput;
use mismatch::Mismatch;
use serde::Serialize;
use thiserror::Error;
use tracing::instrument;
use verification_common::blueprint_contracts;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Initialization(anyhow::Error),
    #[error("Compiler version not found: {0}")]
    VersionNotFound(String),
    #[error("Compilation error: {0:?}")]
    Compilation(Vec<String>),
    #[error("{0}")]
    Internal(anyhow::Error),
    #[error("No contract could be verified with provided data")]
    NoMatchingContracts,
    #[error("Invalid compiler version: {0}")]
    CompilerVersionMismatch(Mismatch<semver::Version>),
}

impl From<BytecodeInitError> for Error {
    fn from(error: BytecodeInitError) -> Self {
        Error::Initialization(anyhow!(error))
    }
}

impl From<compiler::Error> for Error {
    fn from(error: compiler::Error) -> Self {
        match error {
            compiler::Error::VersionNotFound(version) => Error::VersionNotFound(version),
            compiler::Error::Compilation(details) => Error::Compilation(details),
            err => Error::Internal(anyhow!(err)),
        }
    }
}

/// The public structure returned as a result when verification succeeds.
#[derive(Clone, Debug)]
pub struct Success {
    pub compiler_output: CompilerOutput,
    pub compiler_version: compiler::DetailedVersion,
    pub file_path: String,
    pub contract_name: String,
    pub abi: Option<serde_json::Value>,
    pub constructor_args: Option<DisplayBytes>,
    pub local_bytecode_parts: LocalBytecodeParts,
    pub match_type: MatchType,
    pub compilation_artifacts: serde_json::Value,
    pub creation_input_artifacts: serde_json::Value,
    pub deployed_bytecode_artifacts: serde_json::Value,
    pub is_blueprint: bool,
}

pub struct ContractVerifier<'a, C> {
    compilers: &'a Compilers<C>,
    compiler_version: &'a compiler::DetailedVersion,
    verifier: Box<
        dyn base::Verifier<
            Input = (
                CompilerOutput,
                CompilerOutput,
                lossless_compiler_output::CompilerOutput,
            ),
        >,
    >,
    chain_id: Option<String>,
    is_blueprint: bool,
}

impl<'a, C: EvmCompiler> ContractVerifier<'a, C> {
    pub fn new(
        compilers: &'a Compilers<C>,
        compiler_version: &'a compiler::DetailedVersion,
        creation_tx_input: Option<Bytes>,
        deployed_bytecode: Bytes,
        chain_id: Option<String>,
    ) -> Result<Self, Error> {
        let mut is_blueprint = false;
        let verifier: Box<
            dyn base::Verifier<
                Input = (
                    CompilerOutput,
                    CompilerOutput,
                    lossless_compiler_output::CompilerOutput,
                ),
            >,
        > = match creation_tx_input {
            None => {
                if let Some(blueprint_contract) =
                    blueprint_contracts::from_runtime_code(deployed_bytecode.clone())
                {
                    is_blueprint = true;
                    Box::new(all_metadata_extracting_verifier::Verifier::<
                        CreationTxInputWithoutConstructorArgs,
                    >::new(blueprint_contract.initcode)?)
                } else {
                    Box::new(all_metadata_extracting_verifier::Verifier::<
                        DeployedBytecode,
                    >::new(deployed_bytecode)?)
                }
            }
            Some(creation_tx_input) => {
                if let Some(blueprint_contract) =
                    blueprint_contracts::from_creation_code(creation_tx_input.clone())
                {
                    is_blueprint = true;
                    Box::new(all_metadata_extracting_verifier::Verifier::<
                        CreationTxInputWithoutConstructorArgs,
                    >::new(blueprint_contract.initcode)?)
                } else {
                    Box::new(
                        all_metadata_extracting_verifier::Verifier::<CreationTxInput>::new(
                            creation_tx_input,
                        )?,
                    )
                }
            }
        };
        Ok(Self {
            compilers,
            compiler_version,
            verifier,
            chain_id,
            is_blueprint,
        })
    }

    #[instrument(skip(self, compiler_input), level = "debug")]
    pub async fn verify(&self, compiler_input: &C::CompilerInput) -> Result<Success, Error>
    where
        C::CompilerInput: CompilerInput + Serialize + Clone,
    {
        let (raw_compiler_output, compiler_output) = self
            .compilers
            .compile(
                self.compiler_version,
                compiler_input,
                self.chain_id.as_deref(),
            )
            .await?;
        let (_raw_compiler_output_modified, compiler_output_modified) = {
            let compiler_input = compiler_input.clone().modify();
            self.compilers
                .compile(
                    self.compiler_version,
                    &compiler_input,
                    self.chain_id.as_deref(),
                )
                .await?
        };

        let lossless_compiler_output: lossless_compiler_output::CompilerOutput =
            serde_json::from_value(raw_compiler_output)
                .context("could not parse raw compiler output into lossless struct")
                .map_err(Error::Internal)?;

        let outputs = (
            compiler_output,
            compiler_output_modified,
            lossless_compiler_output,
        );
        let verification_success = self.verifier.verify(&outputs).map_err(|errs| {
            errs.into_iter()
                .find_map(|err| match err {
                    // Even one CompilerVersionMismatch error indicates that provided
                    // compiler version does not correspond to on chain bytecode.
                    // We want to notify a user explicitly.
                    //
                    // Notice, that from `VerificationErrorKind` point of view, we compare result of
                    // locally compiled bytecode with the remote bytecode, thus, expected local version
                    // and found the remote. But from `Error::CompilerVersionMismatch` point of view, the remote
                    // version is the actual version we compare with, thus expected the remote version and found
                    // the compiler version provided by the user.
                    VerificationError {
                        kind:
                            VerificationErrorKind::CompilerVersionMismatch(Mismatch {
                                // 'found' contains solc version of the remote bytecode.
                                found: Some(version),
                                ..
                            }),
                        ..
                    } => Some(Error::CompilerVersionMismatch(Mismatch::new(
                        version,
                        self.compiler_version.version().clone(),
                    ))),
                    _ => None,
                })
                .unwrap_or(Error::NoMatchingContracts)
        })?;

        let (_raw_output, compiler_output, _compiler_output_modified) = outputs;
        // We accept compiler input and compiler version by reference, so that we
        // avoid their cloning if verification fails.
        // In case of success, they will be cloned exactly once.
        Ok(Success {
            compiler_output,
            compiler_version: self.compiler_version.clone(),
            file_path: verification_success.file_path,
            contract_name: verification_success.contract_name,
            abi: verification_success.abi,
            constructor_args: verification_success.constructor_args,
            local_bytecode_parts: verification_success.local_bytecode_parts,
            match_type: verification_success.match_type,
            compilation_artifacts: verification_success.compilation_artifacts,
            creation_input_artifacts: verification_success.creation_input_artifacts,
            deployed_bytecode_artifacts: verification_success.deployed_bytecode_artifacts,
            is_blueprint: self.is_blueprint,
        })
    }
}
