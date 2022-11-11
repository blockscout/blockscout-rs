use super::{
    base::{self, VerificationSuccess},
    bytecode::{Bytecode, BytecodePart, LocalBytecode, Source},
    errors::{BytecodeInitError, VerificationError, VerificationErrorKind},
    metadata::MetadataHash,
};
use crate::{mismatch::Mismatch, DisplayBytes};
use bytes::Bytes;
use ethabi::{Constructor, Token};
use ethers_solc::{artifacts::Contract, Artifact, CompilerOutput};

/// Verifier used for contract verification.
///
/// Contains input data provided by the requester that will
/// further be used in verification process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verifier<T> {
    remote_bytecode: Bytecode<T>,
}

impl<T: Source> base::Verifier for Verifier<T> {
    type Input = (CompilerOutput, CompilerOutput);

    fn verify(&self, input: &Self::Input) -> Result<VerificationSuccess, Vec<VerificationError>> {
        self.verify(&input.0, &input.1)
    }
}

impl<T: Source> Verifier<T> {
    pub fn new(input: Bytes) -> Result<Self, BytecodeInitError> {
        let bytecode = Bytecode::new(input)?;
        Ok(Self {
            remote_bytecode: bytecode,
        })
    }

    /// Verifies input data provided on initialization by comparing it
    /// with compiler output received when compiling source data locally.
    ///
    /// Iterates through all contracts received from local compilation and
    /// returns [`VerificationSuccess`] with file path and contract name
    /// of succeeded contract, if any. Otherwise, returns [`None`].
    pub fn verify(
        &self,
        output: &CompilerOutput,
        output_modified: &CompilerOutput,
    ) -> Result<VerificationSuccess, Vec<VerificationError>> {
        let not_found_in_modified_compiler_output_error =
            |file_path: String, contract_name: Option<String>| match contract_name {
                None => VerificationError::new(
                    file_path,
                    VerificationErrorKind::InternalError(
                        "not found in modified compiler output".into(),
                    ),
                ),
                Some(contract_name) => VerificationError::with_contract(
                    file_path,
                    contract_name,
                    VerificationErrorKind::InternalError(
                        "not found in modified compiler output".into(),
                    ),
                ),
            };

        let mut errors = Vec::new();
        for (path, contracts) in &output.contracts {
            let contracts_modified = {
                if let Some(contracts_modified) = output_modified.contracts.get(path) {
                    contracts_modified
                } else {
                    let error = not_found_in_modified_compiler_output_error(path.clone(), None);

                    tracing::error!("{}", error);
                    errors.push(error);

                    continue;
                }
            };

            for (name, contract) in contracts {
                let contract_modified = {
                    if let Some(contract) = contracts_modified.get(name) {
                        contract
                    } else {
                        let error = not_found_in_modified_compiler_output_error(
                            path.clone(),
                            Some(name.clone()),
                        );

                        tracing::error!("{}", error);
                        errors.push(error);

                        continue;
                    }
                };

                match self.compare(contract, contract_modified) {
                    Ok((abi, constructor_args)) => {
                        return Ok(VerificationSuccess {
                            file_path: path.clone(),
                            contract_name: name.clone(),
                            abi,
                            constructor_args: constructor_args.map(DisplayBytes::from),
                        })
                    }
                    Err(err) => {
                        let error =
                            VerificationError::with_contract(path.clone(), name.clone(), err);

                        match error {
                            VerificationError {
                                kind: VerificationErrorKind::InternalError(_),
                                ..
                            } => {
                                tracing::error!("{}", error);
                            }
                            _ => {
                                tracing::debug!("{}", error);
                            }
                        }
                        errors.push(error)
                    }
                }
            }
        }

        Err(errors)
    }

    /// Tries to verify the remote bytecode via locally compiled contract.
    fn compare(
        &self,
        contract: &Contract,
        contract_modified: &Contract,
    ) -> Result<(Option<ethabi::Contract>, Option<Bytes>), VerificationErrorKind> {
        let bytecode = Bytecode::try_from(contract).map_err(|err| match err {
            BytecodeInitError::Empty => VerificationErrorKind::AbstractContract,
            // Corresponding bytecode was not linked properly
            BytecodeInitError::InvalidCreationTxInput(_)
            | BytecodeInitError::InvalidDeployedBytecode(_) => VerificationErrorKind::LibraryMissed,
        })?;
        // If libraries were linked for main contract, they must be linked for modified contract as well
        let bytecode_modified = Bytecode::try_from(contract_modified).map_err(|err| {
            VerificationErrorKind::InternalError(format!("modified contract: {}", err))
        })?;

        let local_bytecode = LocalBytecode::new(bytecode, bytecode_modified)?;

        Self::compare_creation_tx_inputs(&self.remote_bytecode, &local_bytecode)?;

        let abi = contract.get_abi().map(|abi| abi.into_owned());

        let constructor_args = Self::extract_constructor_args(
            self.remote_bytecode.bytecode(),
            local_bytecode.bytecode(),
            abi.as_ref().and_then(|abi| abi.constructor()),
        )?;

        Ok((abi, constructor_args))
    }

    fn compare_creation_tx_inputs(
        remote_bytecode: &Bytecode<T>,
        local_bytecode: &LocalBytecode<T>,
    ) -> Result<(), VerificationErrorKind> {
        let remote_creation_tx_input = remote_bytecode.bytecode();
        let local_creation_tx_input = local_bytecode.bytecode();

        if remote_creation_tx_input.len() < local_creation_tx_input.len() {
            return Err(VerificationErrorKind::BytecodeLengthMismatch {
                part: Mismatch::new(
                    local_creation_tx_input.len(),
                    remote_creation_tx_input.len(),
                ),
                raw: Mismatch::new(
                    local_creation_tx_input.clone().into(),
                    remote_creation_tx_input.clone().into(),
                ),
            });
        }

        Self::compare_bytecode_parts(
            remote_creation_tx_input,
            local_creation_tx_input,
            local_bytecode.bytecode_parts(),
        )?;

        Ok(())
    }

    /// Performs an actual comparison of locally compiled bytecode
    /// with remote bytecode provided for verification.
    ///
    /// # Panics
    ///
    /// The function will panic if `remote_raw.len()` is less than `local_raw.len()`.
    fn compare_bytecode_parts(
        remote_raw: &Bytes,
        local_raw: &Bytes,
        local_parts: &Vec<BytecodePart>,
    ) -> Result<(), VerificationErrorKind> {
        // A caller should ensure that this precondition holds.
        // Currently only `compare_creation_tx_inputs` calls current function,
        // and it guarantees that `remote_creation_tx_input.len() < local_creation_tx_input.len()`
        assert!(
            // if that fails, we would be out of range further anyway
            remote_raw.len() >= local_raw.len(),
            "Local bytecode is greater than remote"
        );

        let mut i = 0usize; // keep track of current processing position of `remote_raw`

        for part in local_parts {
            match part {
                BytecodePart::Main { raw } => {
                    if raw != &remote_raw[i..i + raw.len()] {
                        return Err(VerificationErrorKind::BytecodeMismatch {
                            part: Mismatch::new(
                                raw.clone().into(),
                                remote_raw.slice(i..i + raw.len()).into(),
                            ),
                            raw: Mismatch::new(local_raw.clone().into(), remote_raw.clone().into()),
                        });
                    }
                }
                BytecodePart::Metadata {
                    metadata,
                    metadata_length_raw,
                    ..
                } => {
                    let (remote_metadata, remote_metadata_length) =
                        MetadataHash::from_cbor(&remote_raw[i..])
                            .map_err(|err| VerificationErrorKind::MetadataParse(err.to_string()))?;

                    let start_index = i + remote_metadata_length;
                    if &remote_raw[start_index..start_index + 2] != metadata_length_raw {
                        return Err(VerificationErrorKind::MetadataParse(
                            "metadata length mismatch".into(),
                        ));
                    }

                    // We may say the compiler versions does not correspond to each other only in case if both compiler versions are present.
                    // Otherwise, we cannot say for sure if compiler version is invalid.
                    if let (Some(metadata_solc), Some(remote_metadata_solc)) =
                        (&metadata.solc, &remote_metadata.solc)
                    {
                        if metadata_solc != remote_metadata_solc {
                            let expected_solc = metadata_solc.clone();
                            let remote_solc = remote_metadata_solc.clone();
                            return Err(VerificationErrorKind::CompilerVersionMismatch(
                                Mismatch::new(expected_solc, remote_solc),
                            ));
                        }
                    }
                }
            }

            i += part.size();
        }

        Ok(())
    }

    /// Extracts constructor arguments from the creation transaction input specified on
    /// [`Verifier`] initialization.
    fn extract_constructor_args(
        remote_raw: &Bytes,
        local_raw: &Bytes,
        abi_constructor: Option<&Constructor>,
    ) -> Result<Option<Bytes>, VerificationErrorKind> {
        let encoded_constructor_args = remote_raw.slice(local_raw.len()..);
        let encoded_constructor_args = if encoded_constructor_args.is_empty() {
            None
        } else {
            Some(encoded_constructor_args)
        };

        let expects_constructor_args = T::has_constructor_args() // check that the source actually should have constructor args
                && abi_constructor.map(|input| input.inputs.len()).unwrap_or(0) > 0; // check that the contract itself should have constructor args

        match encoded_constructor_args {
            None if expects_constructor_args => Err(
                VerificationErrorKind::InvalidConstructorArguments(DisplayBytes::from([])),
            ),
            Some(encoded) if !expects_constructor_args => Err(
                VerificationErrorKind::InvalidConstructorArguments(encoded.into()),
            ),
            None => Ok(None),
            Some(encoded_constructor_args) => {
                let _constructor_args = Self::parse_constructor_args(
                    encoded_constructor_args.clone(),
                    abi_constructor.expect("Is not None as `expects_constructor_args`"),
                )?;
                Ok(Some(encoded_constructor_args))
            }
        }
    }

    /// Parses encoded arguments via constructor types specified into abi.
    ///
    /// Returns `Err` if bytes do not correspond to the constructor arguments representation.
    fn parse_constructor_args(
        encoded_args: Bytes,
        abi_constructor: &Constructor,
    ) -> Result<Vec<Token>, VerificationErrorKind> {
        let param_types = |inputs: &Vec<ethabi::Param>| -> Vec<ethabi::ParamType> {
            inputs.iter().map(|p| p.kind.clone()).collect()
        };
        let param_types = param_types(&abi_constructor.inputs);
        let tokens = ethabi::decode(&param_types, encoded_args.as_ref()).map_err(|_err| {
            VerificationErrorKind::InvalidConstructorArguments(encoded_args.into())
        })?;

        Ok(tokens)
    }
}

#[cfg(test)]
mod verifier_initialization_tests {
    use super::{
        super::bytecode::{CreationTxInput, DeployedBytecode},
        *,
    };
    use const_format::concatcp;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    const DEFAULT_CONSTRUCTOR_ARGS: &'static str =
        "0000000000000000000000000000000000000000000000000000000000000fff";
    // {"ipfs": h'1220EB23CE2C13EA8739368F952F6C6A4B1F0623D147D2A19B6D4D26A61AB03FCD3E', "solc": 0.8.14}
    const DEFAULT_ENCODED_METADATA_HASH: &'static str = "a2646970667358221220eb23ce2c13ea8739368f952f6c6a4b1f0623d147d2a19b6d4d26a61ab03fcd3e64736f6c634300080e0033";
    const DEFAULT_BYTECODE_WITHOUT_METADATA_HASH: &'static str = "608060405234801561001057600080fd5b5060405161022038038061022083398101604081905261002f91610074565b600080546001600160a01b0319163390811782556040519091907f342827c97908e5e2f71151c08502a66d44b6f758e3ac2f1de95f02eb95f0a735908290a35061008d565b60006020828403121561008657600080fd5b5051919050565b6101848061009c6000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c8063893d20e81461003b578063a6f9dae11461005a575b600080fd5b600054604080516001600160a01b039092168252519081900360200190f35b61006d61006836600461011e565b61006f565b005b6000546001600160a01b031633146100c35760405162461bcd60e51b815260206004820152601360248201527221b0b63632b91034b9903737ba1037bbb732b960691b604482015260640160405180910390fd5b600080546040516001600160a01b03808516939216917f342827c97908e5e2f71151c08502a66d44b6f758e3ac2f1de95f02eb95f0a73591a3600080546001600160a01b0319166001600160a01b0392909216919091179055565b60006020828403121561013057600080fd5b81356001600160a01b038116811461014757600080fd5b939250505056fe";
    const DEFAULT_DEPLOYED_BYTECODE_WITHOUT_METADATA_HASH: &'static str =  "608060405234801561001057600080fd5b50600436106100365760003560e01c8063893d20e81461003b578063a6f9dae11461005a575b600080fd5b600054604080516001600160a01b039092168252519081900360200190f35b61006d61006836600461011e565b61006f565b005b6000546001600160a01b031633146100c35760405162461bcd60e51b815260206004820152601360248201527221b0b63632b91034b9903737ba1037bbb732b960691b604482015260640160405180910390fd5b600080546040516001600160a01b03808516939216917f342827c97908e5e2f71151c08502a66d44b6f758e3ac2f1de95f02eb95f0a73591a3600080546001600160a01b0319166001600160a01b0392909216919091179055565b60006020828403121561013057600080fd5b81356001600160a01b038116811461014757600080fd5b939250505056fe";

    const DEFAULT_CREATION_TX_INPUT: &'static str = concatcp!(
        DEFAULT_BYTECODE_WITHOUT_METADATA_HASH,
        DEFAULT_ENCODED_METADATA_HASH,
        DEFAULT_CONSTRUCTOR_ARGS
    );
    const DEFAULT_DEPLOYED_BYTECODE: &'static str = concatcp!(
        DEFAULT_DEPLOYED_BYTECODE_WITHOUT_METADATA_HASH,
        DEFAULT_ENCODED_METADATA_HASH
    );

    fn new_verifier<T: Source>(bytecode: &str) -> Result<Verifier<T>, BytecodeInitError> {
        let bytecode = DisplayBytes::from_str(bytecode)
            .expect("Invalid bytecode")
            .0;
        Verifier::new(bytecode)
    }

    #[test]
    fn initialization_with_valid_creation_tx_input() {
        let verifier = new_verifier::<CreationTxInput>(DEFAULT_CREATION_TX_INPUT);
        assert!(
            verifier.is_ok(),
            "Initialization without \"0x\" prefix failed"
        );

        let verifier = new_verifier::<CreationTxInput>(&concatcp!("0x", DEFAULT_CREATION_TX_INPUT));
        assert!(verifier.is_ok(), "Initialization with \"0x\" prefix failed");
    }

    #[test]
    fn initialization_with_valid_deployed_bytecode() {
        let verifier = new_verifier::<DeployedBytecode>(DEFAULT_DEPLOYED_BYTECODE);
        assert!(
            verifier.is_ok(),
            "Initialization without \"0x\" prefix failed"
        );

        let verifier =
            new_verifier::<DeployedBytecode>(&concatcp!("0x", DEFAULT_DEPLOYED_BYTECODE));
        assert!(verifier.is_ok(), "Initialization with \"0x\" prefix failed");
    }

    #[test]
    fn initialization_with_empty_creation_tx_input_should_fail() {
        let verifier = new_verifier::<CreationTxInput>("");
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(verifier.unwrap_err(), BytecodeInitError::Empty,)
    }

    #[test]
    fn initialization_with_empty_deployed_bytecode_should_fail() {
        let verifier = new_verifier::<DeployedBytecode>("");
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(verifier.unwrap_err(), BytecodeInitError::Empty)
    }
}
