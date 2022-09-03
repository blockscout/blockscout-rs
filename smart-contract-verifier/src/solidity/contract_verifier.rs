use super::{
    compiler::SolidityCompiler,
    verifier::{VerificationSuccess, Verifier},
};
use crate::{
    compilers::{self, Compilers, Version},
    solidity::errors::BytecodeInitError,
};
use bytes::Bytes;
use ethers_solc::CompilerInput;
use std::{ops::Add, path::PathBuf};
use thiserror::Error;
use tracing::instrument;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Initialization(#[from] BytecodeInitError),
    #[error("{0}")]
    Compilation(#[from] compilers::Error),
    #[error("No contract could be verified with provided data")]
    NoMatchingContracts,
}

pub struct ContractVerifier<'a> {
    compilers: Compilers<SolidityCompiler>,
    compiler_version: &'a Version,
    verifier: Verifier,
}

impl<'a> ContractVerifier<'a> {
    pub fn new(
        compilers: Compilers<SolidityCompiler>,
        compiler_version: &'a Version,
        creation_tx_input: Bytes,
        deployed_bytecode: Bytes,
    ) -> Result<Self, Error> {
        let verifier = Verifier::from_bytes(creation_tx_input, deployed_bytecode)?;
        Ok(Self {
            compilers,
            compiler_version,
            verifier,
        })
    }

    #[instrument(skip(self, compiler_input), level = "debug")]
    pub async fn verify(
        &self,
        compiler_input: &CompilerInput,
    ) -> Result<VerificationSuccess, Error> {
        let compiler_output = self
            .compilers
            .compile(self.compiler_version, &compiler_input)
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
                .compile(&self.compiler_version, &compiler_input)
                .await?
        };
        self.verifier
            .verify(compiler_output, compiler_output_modified)
            .map_err(|_err| Error::NoMatchingContracts)
    }
}
