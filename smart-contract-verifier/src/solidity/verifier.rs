use super::errors::{BytecodeInitializationError, VerificationError};
use crate::{
    solidity::{errors::VerificationErrorKind, metadata::MetadataHash},
    types::Mismatch,
    DisplayBytes,
};
use bytes::{Buf, Bytes};
use ethabi::{Constructor, Token};
use ethers_solc::{artifacts::Contract, Artifact, CompilerOutput};
use std::str::FromStr;

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
        if creation_tx_input.is_empty() {
            return Err(BytecodeInitializationError::EmptyCreationTxInput);
        }

        let deployed_bytecode = DisplayBytes::from_str(deployed_bytecode)
            .map_err(|_| {
                BytecodeInitializationError::InvalidDeployedBytecode(deployed_bytecode.to_string())
            })?
            .0;
        if deployed_bytecode.is_empty() {
            return Err(BytecodeInitializationError::EmptyDeployedBytecode);
        }

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

        // if !creation_tx_input_str.contains(&deployed_bytecode_str) {
        //     return Err(BytecodeInitializationError::BytecodeMismatch(
        //         Mismatch::new(
        //             DisplayBytes::from(creation_tx_input),
        //             DisplayBytes::from(deployed_bytecode),
        //         ),
        //     ));
        // }

        Ok(Self {
            creation_tx_input,
            deployed_bytecode,

            creation_tx_input_str,
            deployed_bytecode_str,
        })
    }
}

impl TryFrom<&Contract> for Bytecode {
    type Error = BytecodeInitializationError;

    fn try_from(contract: &Contract) -> Result<Self, Self::Error> {
        let deployed_bytecode = {
            contract.get_deployed_bytecode_bytes().ok_or_else(|| {
                let bytecode = contract
                    .get_deployed_bytecode_object()
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                BytecodeInitializationError::InvalidDeployedBytecode(bytecode)
            })?
        };
        let creation_tx_input = {
            contract.get_bytecode_bytes().ok_or_else(|| {
                let bytecode = contract
                    .get_bytecode_object()
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                BytecodeInitializationError::InvalidCreationTxInput(bytecode)
            })?
        };
        Bytecode::from_bytes(creation_tx_input.0.clone(), deployed_bytecode.0.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BytecodePart {
    Main {
        raw: Bytes,
    },
    Metadata {
        metadata_raw: Bytes,
        metadata: MetadataHash,
        metadata_length_raw: Bytes,
    },
}

impl BytecodePart {
    pub fn size(&self) -> usize {
        match self {
            BytecodePart::Main { raw } => raw.len(),
            BytecodePart::Metadata {
                metadata_raw,
                metadata_length_raw,
                ..
            } => metadata_raw.len() + metadata_length_raw.len(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RemoteBytecode {
    bytecode: Bytecode,
}

impl RemoteBytecode {
    pub fn new(bytecode: Bytecode) -> Self {
        Self { bytecode }
    }

    pub fn creation_tx_input(&self) -> &Bytes {
        &self.bytecode.creation_tx_input
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalBytecode {
    bytecode: Bytecode,
    creation_tx_input_parts: Vec<BytecodePart>,
    deployed_bytecode_parts: Vec<BytecodePart>,
}

impl LocalBytecode {
    pub fn new(
        bytecode: Bytecode,
        bytecode_modified: Bytecode,
    ) -> Result<Self, VerificationErrorKind> {
        let creation_tx_input_parts = Self::split(
            bytecode.creation_tx_input.clone(),
            &bytecode_modified.creation_tx_input,
        )?;
        let deployed_bytecode_parts = Self::split(
            bytecode.deployed_bytecode.clone(),
            &bytecode_modified.deployed_bytecode,
        )?;
        // if deployed_bytecode_parts.len() > 2 {
        //     return Err(VerificationErrorKind::InternalError(
        //         "deployed bytecode part contains more than two parts".into(),
        //     ));
        // }

        Ok(Self {
            bytecode,
            creation_tx_input_parts,
            deployed_bytecode_parts,
        })
    }

    pub fn creation_tx_input(&self) -> &Bytes {
        &self.bytecode.creation_tx_input
    }

    fn split(
        mut raw: Bytes,
        raw_modified: &Bytes,
    ) -> Result<Vec<BytecodePart>, VerificationErrorKind> {
        if raw.len() != raw_modified.len() {
            return Err(VerificationErrorKind::InternalError(format!(
                "bytecode and modified bytecode length mismatch: {}",
                Mismatch::new(raw.len(), raw_modified.len())
            )));
        }

        let parts_total_size = |parts: &Vec<BytecodePart>| -> usize {
            parts.iter().fold(0, |size, el| size + el.size())
        };

        let mut result = Vec::new();

        let mut i = 0usize;
        while !raw.is_empty() {
            let decoded = Self::parse_bytecode_parts(&raw, &raw_modified[i..])?;
            let decoded_size = parts_total_size(&decoded);
            result.extend(decoded);

            raw.advance(decoded_size);
            i += decoded_size;
        }

        Ok(result)
    }

    fn parse_bytecode_parts(
        raw: &Bytes,
        raw_modified: &[u8],
    ) -> Result<Vec<BytecodePart>, VerificationErrorKind> {
        let mut parts = Vec::new();

        let len = raw.len();

        let mut i = 0usize;
        while i < len {
            if raw[i] == raw_modified[i] {
                i += 1;
                continue;
            }

            // The first different byte. The metadata hash itself started somewhere earlier
            // (at least for "a1"/"a2" indicating number of elements in cbor mapping).
            // Next steps are trying to find that beginning.

            let mut result = MetadataHash::from_cbor(&raw[i..]);
            while result.is_err() {
                // It is the beginning of the bytecode segment but no metadata hash has been parsed
                if i == 0 {
                    return Err(VerificationErrorKind::InternalError(
                        "failed to parse bytecode part".into(),
                    ));
                }
                i -= 1;

                result = MetadataHash::from_cbor(&raw[i..]);
            }

            let (metadata, metadata_length) = result.unwrap();

            if len < i + metadata_length + 2 {
                return Err(VerificationErrorKind::InternalError(
                    "failed to parse metadata length".into(),
                ));
            }
            // Decode length of metadata hash representation
            let metadata_length_raw = raw.slice((i + metadata_length)..(i + metadata_length + 2));
            let encoded_metadata_length = metadata_length_raw.clone().get_u16() as usize;
            if encoded_metadata_length != metadata_length {
                return Err(VerificationErrorKind::InternalError(
                    "encoded metadata length does not correspond to actual metadata length".into(),
                ));
            }

            parts.push(BytecodePart::Metadata {
                metadata_raw: raw.slice(i..(i + metadata_length)),
                metadata,
                metadata_length_raw,
            });
            break;
        }

        if i > 0 {
            parts.insert(
                0,
                BytecodePart::Main {
                    raw: raw.slice(0..i),
                },
            )
        }

        Ok(parts)
    }
}

/// Verifier used for contract verification.
///
/// Contains input data provided by the requester that will
/// further be used in verification process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verifier {
    remote_bytecode: RemoteBytecode,
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
        let bytecode = Bytecode::new(creation_tx_input, deployed_bytecode)?;
        Ok(Self {
            remote_bytecode: RemoteBytecode::new(bytecode),
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
        output: CompilerOutput,
        output_modified: CompilerOutput,
    ) -> Result<VerificationSuccess, Vec<VerificationError>> {
        let mut errors = Vec::new();
        for (path, contracts) in output.contracts {
            let contracts_modified = {
                if let Some(contracts_modified) = output_modified.contracts.get(&path) {
                    contracts_modified
                } else {
                    let error = VerificationError::new(
                        path,
                        VerificationErrorKind::InternalError(
                            "not found in modified compiler output".into(),
                        ),
                    );

                    tracing::error!("{}", error);
                    errors.push(error);

                    continue;
                }
            };

            for (name, contract) in contracts {
                let contract_modified = {
                    if let Some(contract) = contracts_modified.get(&name) {
                        contract
                    } else {
                        let error = VerificationError::with_contract(
                            path.clone(),
                            name,
                            VerificationErrorKind::InternalError(
                                "not found in modified compiler output".into(),
                            ),
                        );

                        tracing::error!("{}", error);
                        errors.push(error);

                        continue;
                    }
                };

                match self.compare(&contract, contract_modified) {
                    Ok((abi, constructor_args)) => {
                        return Ok(VerificationSuccess {
                            file_path: path,
                            contract_name: name,
                            abi,
                            constructor_args: constructor_args.map(DisplayBytes::from),
                        })
                    }
                    Err(err) => {
                        let error = VerificationError::with_contract(path.clone(), name, err);

                        tracing::error!("{}", error);
                        errors.push(error)
                    }
                }
            }
        }

        Err(errors)
    }

    fn compare(
        &self,
        contract: &Contract,
        contract_modified: &Contract,
    ) -> Result<(ethabi::Contract, Option<Bytes>), VerificationErrorKind> {
        let abi = contract
            .get_abi()
            .ok_or_else(|| VerificationErrorKind::InternalError("missing abi".into()))?;

        let bytecode = Bytecode::try_from(contract).map_err(|err| match err {
            BytecodeInitializationError::EmptyCreationTxInput
            | BytecodeInitializationError::EmptyDeployedBytecode => {
                VerificationErrorKind::AbstractContract
            }
            // Corresponding bytecode was not linked properly
            BytecodeInitializationError::InvalidCreationTxInput(_)
            | BytecodeInitializationError::InvalidDeployedBytecode(_) => {
                VerificationErrorKind::LibraryMissed
            }
        })?;
        // If libraries were linked for main contract, they must be linked for modified contract as well
        let bytecode_modified = Bytecode::try_from(contract_modified).map_err(|err| {
            VerificationErrorKind::InternalError(format!("modified contract: {}", err))
        })?;

        let local_bytecode = LocalBytecode::new(bytecode, bytecode_modified)?;

        Self::compare_creation_tx_inputs(&self.remote_bytecode, &local_bytecode)?;

        let constructor_args = Self::extract_constructor_args(
            self.remote_bytecode.creation_tx_input(),
            local_bytecode.creation_tx_input(),
            abi.constructor(),
        )?;

        Ok((abi.into_owned(), constructor_args))
    }

    fn compare_creation_tx_inputs(
        remote_bytecode: &RemoteBytecode,
        local_bytecode: &LocalBytecode,
    ) -> Result<(), VerificationErrorKind> {
        let remote_creation_tx_input = remote_bytecode.creation_tx_input();
        let local_creation_tx_input = local_bytecode.creation_tx_input();

        if remote_creation_tx_input.len() < local_creation_tx_input.len() {
            return Err(VerificationErrorKind::BytecodeMismatch(Mismatch::new(
                local_creation_tx_input.clone().into(),
                remote_creation_tx_input.clone().into(),
            )));
        }

        Self::compare_bytecode_parts(
            remote_creation_tx_input,
            local_creation_tx_input,
            &local_bytecode.creation_tx_input_parts,
        )?;

        Ok(())
    }

    fn compare_bytecode_parts(
        remote_raw: &Bytes,
        local_raw: &Bytes,
        local_parts: &Vec<BytecodePart>,
    ) -> Result<(), VerificationErrorKind> {
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
                        return Err(VerificationErrorKind::BytecodeMismatch(Mismatch::new(
                            local_raw.clone().into(),
                            remote_raw.clone().into(),
                        )));
                    }
                }
                BytecodePart::Metadata {
                    metadata,
                    metadata_length_raw,
                    ..
                } => {
                    let (remote_metadata, remote_metadata_size) =
                        MetadataHash::from_cbor(&remote_raw[i..])
                            .map_err(|err| VerificationErrorKind::MetadataParse(err.to_string()))?;

                    let start_index = i + remote_metadata_size;
                    if &remote_raw[start_index..start_index + 2] != metadata_length_raw {
                        return Err(VerificationErrorKind::MetadataParse(
                            "metadata length mismatch".into(),
                        ));
                    }

                    if metadata.solc != remote_metadata.solc {
                        let expected_solc = metadata
                            .solc
                            .as_ref()
                            .map(|b| DisplayBytes::from(b.clone()).to_string());
                        let remote_solc = remote_metadata
                            .solc
                            .as_ref()
                            .map(|b| DisplayBytes::from(b.clone()).to_string());
                        return Err(VerificationErrorKind::CompilerVersionMismatch(
                            Mismatch::new(expected_solc, remote_solc),
                        ));
                    }
                }
            }

            i += part.size();
        }

        Ok(())
    }

    /// Extracts constructor arguments from the creation transaction input specified on
    /// [`Verifier`] initialization.
    ///
    /// Returns `Err` if constructor arguments cannot be extracted (should not be the case
    /// if `Bytecode.verify_bytecode_with_extra_data` was called before).
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

        let expects_constructor_args =
            abi_constructor.map(|input| input.inputs.len()).unwrap_or(0) > 0;

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
        // &self,
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
    use super::*;
    use const_format::concatcp;
    use pretty_assertions::assert_eq;

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

    #[test]
    fn initialization_with_valid_data() {
        let verifier = Verifier::new(DEFAULT_CREATION_TX_INPUT, DEFAULT_DEPLOYED_BYTECODE);
        assert!(
            verifier.is_ok(),
            "Initialization without \"0x\" prefix failed"
        );

        let verifier = Verifier::new(
            &concatcp!("0x", DEFAULT_CREATION_TX_INPUT),
            &concatcp!("0x", DEFAULT_DEPLOYED_BYTECODE),
        );
        assert!(verifier.is_ok(), "Initialization with \"0x\" prefix failed");
    }

    #[test]
    fn initialization_with_empty_creation_tx_input_should_fail() {
        let verifier = Verifier::new("", DEFAULT_DEPLOYED_BYTECODE);
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            BytecodeInitializationError::EmptyCreationTxInput,
        )
    }

    #[test]
    fn initialization_with_invalid_hex_as_creation_tx_input_should_fail() {
        let invalid_input = "0xabcdefghij";
        let verifier = Verifier::new(invalid_input, DEFAULT_DEPLOYED_BYTECODE);
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            BytecodeInitializationError::InvalidCreationTxInput(invalid_input.into()),
        )
    }

    #[test]
    fn initialization_with_empty_deployed_bytecode_should_fail() {
        let verifier = Verifier::new(DEFAULT_CREATION_TX_INPUT, "");
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            BytecodeInitializationError::EmptyDeployedBytecode
        )
    }

    #[test]
    fn initialization_with_invalid_hex_as_deployed_bytecode_should_fail() {
        let invalid_input = "0xabcdefghij";
        let verifier = Verifier::new(DEFAULT_CREATION_TX_INPUT, invalid_input);
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            BytecodeInitializationError::InvalidDeployedBytecode(invalid_input.into())
        )
    }
}
