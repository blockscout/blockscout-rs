#![allow(dead_code, unused)]

use crate::{types::Mismatch, DisplayBytes};
use bytes::Bytes;
use ethers_solc::CompilerOutput;
use std::str::FromStr;
use thiserror::Error;

/// Errors that may occur during deployed bytecode and
/// creation transaction input parsing.
#[derive(Error, Clone, Debug, PartialEq, Eq)]
pub enum BytecodeInitializationError {
    #[error("creation transaction input is not a valid hex string: {0}")]
    InvalidCreationTxInput(String),
    #[error("deployed bytecode is not a valid hex string: {0}")]
    InvalidDeployedBytecode(String),
    #[error("deployed bytecode does not correspond to creation tx input: {0}")]
    BytecodeMismatch(Mismatch<DisplayBytes>),
}

/// Combine creation_tx_input and deployed_bytecode.
/// Guarantees that `deployed_bytecode` was actually deployed
/// by `creation_tx_input`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Bytecode {
    /// Raw bytecode bytes used in contract creation transaction
    creation_tx_input: Bytes,
    /// Raw deployed bytecode bytes
    deployed_bytecode: Bytes,

    /// Hex representation of creation tx input without "0x" prefix
    creation_tx_input_str: String,
    /// Hex representation of deployed bytecode without "0x" prefix
    deployed_bytecode_str: String,
}

impl Bytecode {
    pub fn new(
        creation_tx_input: &str,
        deployed_bytecode: &str,
    ) -> Result<Self, BytecodeInitializationError> {
        let creation_tx_input = DisplayBytes::from_str(creation_tx_input)
            .map_err(|_| {
                BytecodeInitializationError::InvalidCreationTxInput(creation_tx_input.to_string())
            })?
            .0;
        let deployed_bytecode = DisplayBytes::from_str(deployed_bytecode)
            .map_err(|_| {
                BytecodeInitializationError::InvalidDeployedBytecode(deployed_bytecode.to_string())
            })?
            .0;

        // We transform `&str` into `Bytes` initially, so that we can ensure that initial input is a valid hex string
        // and enforce internal `Bytecode` representation not to have `0x` prefix in it.
        Self::from_bytes(creation_tx_input, deployed_bytecode)
    }

    pub fn from_bytes(
        creation_tx_input: Bytes,
        deployed_bytecode: Bytes,
    ) -> Result<Self, BytecodeInitializationError> {
        // `hex::encode` encodes bytes as hex string without "0x" prefix
        let creation_tx_input_str = hex::encode(&creation_tx_input);
        let deployed_bytecode_str = hex::encode(&deployed_bytecode);

        if !creation_tx_input_str.contains(&deployed_bytecode_str) {
            return Err(BytecodeInitializationError::BytecodeMismatch(
                Mismatch::new(
                    DisplayBytes::from(creation_tx_input),
                    DisplayBytes::from(deployed_bytecode),
                ),
            ));
        }

        Ok(Self {
            creation_tx_input,
            deployed_bytecode,

            creation_tx_input_str,
            deployed_bytecode_str,
        })
    }
}

/// Verifier used for contract verification.
///
/// Contains input data provided by the requester that will
/// further be used in verification process.
pub struct Verifier {
    remote_bytecode: Bytecode,
}

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
    ) -> Result<Self, BytecodeInitializationError> {
        let remote_bytecode = Bytecode::new(creation_tx_input, deployed_bytecode)?;
        Ok(Self { remote_bytecode })
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
