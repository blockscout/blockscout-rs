use super::{base_verifier::Verifier, errors::BytecodeInitError};
use crate::{
    compiler::{self, Compilers, EvmCompiler, Version},
    DisplayBytes,
};
use anyhow::anyhow;
use bytes::Bytes;
use ethers_solc::CompilerInput;
use std::{ops::Add, path::PathBuf, sync::Arc};
use thiserror::Error;
use tracing::instrument;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Initialization(anyhow::Error),
    #[error("Compiler version not found: {0}")]
    VersionNotFound(Version),
    #[error("Compilation error: {0:?}")]
    Compilation(Vec<String>),
    #[error("{0}")]
    Internal(anyhow::Error),
    #[error("No contract could be verified with provided data")]
    NoMatchingContracts,
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
    pub compiler_input: CompilerInput,
    pub compiler_version: Version,
    pub file_path: String,
    pub contract_name: String,
    pub abi: ethabi::Contract,
    pub constructor_args: Option<DisplayBytes>,
}

pub struct ContractVerifier<'a, T> {
    compilers: Arc<Compilers<T>>,
    compiler_version: &'a Version,
    verifier: Verifier,
}

impl<'a, T: EvmCompiler> ContractVerifier<'a, T> {
    pub fn new(
        compilers: Arc<Compilers<T>>,
        compiler_version: &'a Version,
        creation_tx_input: Bytes,
        deployed_bytecode: Bytes,
    ) -> Result<Self, Error> {
        let verifier = Verifier::new(creation_tx_input, deployed_bytecode)?;
        Ok(Self {
            compilers,
            compiler_version,
            verifier,
        })
    }

    #[instrument(skip(self, compiler_input), level = "debug")]
    pub async fn verify(&self, compiler_input: &CompilerInput) -> Result<Success, Error> {
        let compiler_output = self
            .compilers
            .compile(self.compiler_version, compiler_input)
            .await?;
        let compiler_output_modified = {
            let mut compiler_input = compiler_input.clone();
            let entry = compiler_input
                .settings
                .libraries
                .libs
                .entry(PathBuf::from("SOME_TEXT_USED_AS_FILE_NAME"))
                .or_default();
            let non_used_contract_name = entry
                .keys()
                .map(|key| key.chars().next().unwrap_or_default().to_string())
                .collect::<Vec<_>>()
                .join("_")
                .add("_");
            entry.insert(
                non_used_contract_name,
                "0xcafecafecafecafecafecafecafecafecafecafe".into(),
            );
            self.compilers
                .compile(self.compiler_version, &compiler_input)
                .await?
        };

        let verification_success = self
            .verifier
            .verify(compiler_output, compiler_output_modified)
            .map_err(|_err| Error::NoMatchingContracts)?;

        // We accept compiler input and compiler version by reference, so that we
        // avoid their cloning if verification fails.
        // In case of success, they will be cloned exactly once.
        Ok(Success {
            compiler_input: compiler_input.clone(),
            compiler_version: self.compiler_version.clone(),
            file_path: verification_success.file_path,
            contract_name: verification_success.contract_name,
            abi: verification_success.abi,
            constructor_args: verification_success.constructor_args,
        })
    }
}
