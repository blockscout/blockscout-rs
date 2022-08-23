#![allow(dead_code, unused)]

use crate::{types::Mismatch, DisplayBytes};
use ethers_solc::CompilerOutput;
use thiserror::Error;

pub struct Verifier {}

/// Errors that may occur during deployed bytecode and
/// creation transaction input parsing.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum InitializationError {}

/// The structure returned as a result when verification successes.
/// Contains data needed to be sent back as a verification response.
#[derive(Clone, Debug, PartialEq)]
pub struct VerificationSuccess {
    pub file_path: String,
    pub contract_name: String,
    pub abi: ethabi::Contract,
    pub constructor_args: Option<DisplayBytes>,
}

impl Verifier {
    /// Instantiates a new verifier instance with input data provided by the requester.
    ///
    /// Returns [`InitializationError`] inside [`Err`] if either
    /// `deployed_bytecode` or `creation_tx_input` are invalid or incompatible.
    pub fn new(
        creation_tx_input: &str,
        deployed_bytecode: &str,
    ) -> Result<Self, InitializationError> {
        Ok(Self {})
    }

    /// Verifies input data provided on initialization by comparing it
    /// with compiler output received when compiling source data locally.
    ///
    /// Iterates through all contracts received from local compilation and
    /// returns [`VerificationSuccess`] with file path and contract name
    /// of succeeded contract, if any. Otherwise, returns [`None`].
    pub fn verify(
        &self,
        output: CompilerOutput,
        output_modified: CompilerOutput,
    ) -> Option<VerificationSuccess> {
        todo!()
    }
}
