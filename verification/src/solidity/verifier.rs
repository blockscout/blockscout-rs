#![allow(dead_code, unused)]

use ethers_solc::CompilerOutput;
use std::str::FromStr;
use thiserror::Error;

/// Errors that may occur during initial [`Verifier`] setup
/// with input data provided by the requester.
#[derive(Clone, Debug, Error)]
pub(crate) enum InitializationError {}

/// Errors that may occur during bytecode comparison step.
#[derive(Clone, Debug, Error)]
pub(crate) enum VerificationError {}

/// Wrapper under `evm.deployedBytecode` from the standard output JSON
/// (https://docs.soliditylang.org/en/latest/using-the-compiler.html#output-description).
///
/// Provides an interface to retrieve parts the deployed bytecode consists of:
/// actual bytecode participating in EVM transaction execution and optionally metadata hash.
#[derive(Clone, Debug, PartialEq)]
struct DeployedBytecode {}

impl FromStr for DeployedBytecode {
    type Err = InitializationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

/// Wrapper under `evm.bytecode.object` from the standard output JSON
/// (https://docs.soliditylang.org/en/latest/using-the-compiler.html#output-description)
/// excluding metadata hash and optionally including constructor arguments used on a contract creation.
#[derive(Clone, Debug, PartialEq)]
struct BytecodeWithConstructorArgs {}

impl BytecodeWithConstructorArgs {
    /// Initializes the structure from string and parsed deployed bytecode.
    /// It extracts metadata hash from the provided string and extracts
    /// constructor arguments used on a contract creation if possible.
    ///
    /// Deployed bytecode is required to extract metadata hash from the string.
    pub fn from_str(
        s: &str,
        deployed_bytecode: &DeployedBytecode,
    ) -> Result<Self, InitializationError> {
        todo!()
    }
}

/// Verifier used in contract verification.
///
/// Contains input data provided by the requester that will
/// further be used in verification process.
#[derive(Clone, Debug)]
pub(crate) struct Verifier {
    /// Name of the contract to be verified
    contract_name: String,
    /// File path contract to be verified is located at
    file_path: String,
    /// Bytecode used on the contract creation transaction
    bc_creation_tx_input: BytecodeWithConstructorArgs,
    /// Bytecode stored in the chain and being used by EVM
    bc_deployed_bytecode: DeployedBytecode,
}

impl Verifier {
    /// Instantiates a new verifier instance with input data provided by the requester.
    ///
    /// Returns [`InitializationError`] inside [`Err`] if either `deployed_bytecode` or `creation_tx_input` are invalid.
    pub fn new(
        contract_name: String,
        file_path: String,
        creation_tx_input: &str,
        deployed_bytecode: &str,
    ) -> Result<Self, InitializationError> {
        let deployed_bytecode = DeployedBytecode::from_str(deployed_bytecode)?;
        let bytecode =
            BytecodeWithConstructorArgs::from_str(creation_tx_input, &deployed_bytecode)?;

        Ok(Self {
            contract_name,
            file_path,
            bc_deployed_bytecode: deployed_bytecode,
            bc_creation_tx_input: bytecode,
        })
    }

    /// Verifies input data provided on initialization by comparing it
    /// with compiler output received when compiling source data locally.
    ///
    /// If verification succeeds return [`Ok`], otherwise when verification
    /// fails return an [`VerificationError`] inside [`Err`].
    pub fn verify(&self, output: CompilerOutput) -> Result<(), VerificationError> {
        todo!()
    }
}
