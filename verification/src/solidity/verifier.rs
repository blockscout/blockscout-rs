#![allow(dead_code, unused)]

use crate::{types::Mismatch, DisplayBytes};
use bytes::{Buf, Bytes};
use ethabi::{Constructor, Token};
use ethers_solc::{artifacts::Contract, Artifact, CompilerOutput};
use minicbor::{data::Type, Decode, Decoder};
use std::{
    error::Error,
    fmt::{Debug, Formatter},
    str::FromStr,
};
use thiserror::Error;

/// Errors that may occur during initial [`Verifier`] setup
/// with input data provided by the requester.
#[derive(Clone, Debug, PartialEq, Error)]
pub(crate) enum InitializationError {
    #[error("creation transaction input is not a valid hex string")]
    InvalidCreationTxInput(String),
    #[error("deployed bytecode is not a valid hex string: {0}")]
    InvalidDeployedBytecode(String),
    #[error("cannot parse metadata hash from deployed bytecode: {0}")]
    MetadataHashParse(String),
    #[error("creation transaction input has different metadata hash to deployed bytecode: {0}")]
    MetadataHashMismatch(Mismatch<DisplayBytes>),
}

/// Errors that may occur during bytecode comparison step.
#[derive(Clone, Debug, Error)]
enum VerificationError {
    #[error("deployed bytecode is invalid (most probably the contract is abstract and has no deployed bytecode): {0}")]
    InvalidDeployedBytecode(String),
    #[error("compiler versions included into metadata hash does not match: {0:?}")]
    CompilerVersionMismatch(Mismatch<Option<String>>),
    #[error("bytecode does not match compilation output: {0}")]
    BytecodeMismatch(Mismatch<DisplayBytes>),
    #[error("extra data after metadata hash but before constructor args does not match compilation output: {0}")]
    ExtraDataMismatch(Mismatch<DisplayBytes>),
    #[error("invalid constructor arguments: {0}")]
    InvalidConstructorArguments(DisplayBytes),
    #[error("library missed")]
    MissedLibrary,
    #[error("internal error: {0}")]
    InternalError(String),
}

/// The structure returned as a result when verification successes.
/// Contains data needed to be sent back as a verification response.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VerificationSuccess {
    pub file_path: String,
    pub contract_name: String,
    pub abi: ethabi::Contract,
    pub constructor_args: Option<DisplayBytes>,
}

/// Parsed metadata hash
/// (https://docs.soliditylang.org/en/v0.8.14/metadata.html#encoding-of-the-metadata-hash-in-the-bytecode).
///
/// Currently we are interested only in `solc` value.
#[derive(Clone, Debug, PartialEq)]
struct MetadataHash {
    solc: Option<bytes::Bytes>,
}

impl MetadataHash {
    fn from_cbor(encoded: bytes::Bytes) -> Result<Self, minicbor::decode::Error> {
        minicbor::decode(encoded.as_ref())
    }
}

#[derive(Debug, Error)]
enum ParseMetadataHashError {
    #[error("buffer was not exhausted after all map elements had been processed")]
    NonExhausted,
    #[error("invalid solc type. Expected \"string\" or \"bytes\", found \"{0}\"")]
    InvalidSolcType(Type),
    #[error("\"solc\" key met more than once")]
    DuplicateKeys,
}

impl<'b, C> Decode<'b, C> for MetadataHash {
    fn decode(d: &mut Decoder<'b>, _ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        use minicbor::decode::Error;

        let number_of_elements = d.map()?.unwrap_or(u64::MAX);

        let mut solc = None;
        for _ in 0..number_of_elements {
            // try to parse the key
            match d.str() {
                Ok(s) if s == "solc" => {
                    if solc.is_some() {
                        // duplicate keys are not allowed in CBOR (RFC 8949)
                        return Err(Error::custom(ParseMetadataHashError::DuplicateKeys));
                    }
                    solc = match d.datatype()? {
                        Type::Bytes => Some(d.bytes()?),
                        Type::String => {
                            let s = d.str()?;
                            Some(s.as_bytes())
                        }
                        type_ => {
                            // value of "solc" key must be either String or Bytes
                            return Err(Error::custom(ParseMetadataHashError::InvalidSolcType(
                                type_,
                            )));
                        }
                    }
                }
                Ok(_) => {
                    // if key is not "solc" str we may skip the corresponding value
                    d.skip()?;
                }
                Err(err) if err.is_type_mismatch() => {
                    // if key is not `str` we may skip the corresponding value
                    d.skip()?;
                }
                Err(err) => return Err(err),
            }
        }

        // We require that no elements left in the decoder when
        // the whole map has been processed. That adds another layer
        // of assurance that encoded bytes are actually metadata hash.
        if d.datatype().is_ok() {
            return Err(Error::custom(ParseMetadataHashError::NonExhausted));
        }

        let solc = solc.map(bytes::Bytes::copy_from_slice);
        Ok(MetadataHash { solc })
    }

    fn nil() -> Option<Self> {
        Some(Self { solc: None })
    }
}

/// Wrapper under `evm.deployedBytecode` from the standard output JSON
/// (https://docs.soliditylang.org/en/latest/using-the-compiler.html#output-description).
///
/// Provides an interface to retrieve parts the deployed bytecode consists of:
/// actual bytecode participating in EVM transaction execution and metadata hash.
#[derive(Clone, Debug, PartialEq)]
struct DeployedBytecode {
    /// Bytecode without metadata hash
    bytecode: bytes::Bytes,
    /// Metadata hash encoded into bytecode
    metadata_hash: MetadataHash,
    /// Raw deployed bytecode bytes
    bytes: bytes::Bytes,
}

impl DeployedBytecode {
    /// Returns deployed bytecode without metadata hash
    pub fn bytecode(&self) -> bytes::Bytes {
        self.bytecode.clone()
    }

    /// Returns a metadata hash
    pub fn metadata_hash(&self) -> &MetadataHash {
        &self.metadata_hash
    }

    /// Returns metadata hash encoded as bytes and concatenated with 2 bytes representing its length
    pub fn encoded_metadata_hash_with_length(&self) -> bytes::Bytes {
        let start = self.bytecode.len();
        let end = self.bytes.len();
        self.bytes.slice(start..end)
    }
}

impl FromStr for DeployedBytecode {
    type Err = InitializationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = DisplayBytes::from_str(s)
            .map_err(|_| InitializationError::InvalidDeployedBytecode(s.to_string()))?
            .0;

        DeployedBytecode::try_from(bytes)
    }
}

impl TryFrom<bytes::Bytes> for DeployedBytecode {
    type Error = InitializationError;

    fn try_from(encoded: bytes::Bytes) -> Result<Self, Self::Error> {
        // If metadata is present, last two bytes encode its length in a two-byte big-endian encoding
        if encoded.len() < 2 {
            return Err(InitializationError::MetadataHashParse(
                "length is not encoded".to_string(),
            ));
        }

        // Further we will cut bytes from encoded representation, but original raw
        // bytes are still required for `DeployedBytecode.bytes`
        // (cloning takes O(1) due to internal `bytes::Bytes` implementation)
        let mut b = encoded.clone();

        // Decode length of metadata hash representation
        let metadata_hash_length = {
            let mut length_bytes = b.split_off(b.len() - 2);
            length_bytes.get_u16() as usize
        };

        if b.len() < metadata_hash_length {
            return Err(InitializationError::MetadataHashParse(
                "specified metadata hash length is greater than bytecode total size".to_string(),
            ));
        }

        // Now decode the metadata hash itself
        let metadata_hash = {
            let b_len = b.len();
            let encoded_metadata_hash = b.split_off(b_len - metadata_hash_length);
            MetadataHash::from_cbor(encoded_metadata_hash)
        };

        if let Err(err) = metadata_hash {
            let message = if err.is_custom() {
                format!(
                    "{}",
                    err.source()
                        .expect("`minicbor::decode::Error::Custom` always contains the source")
                )
            } else {
                format!("{}", err)
            };
            return Err(InitializationError::MetadataHashParse(message));
        }

        Ok(Self {
            bytecode: b,
            metadata_hash: metadata_hash.unwrap(),
            bytes: encoded,
        })
    }
}

/// Marker type under [`Bytecode`] indicating that the struct was obtained from creation transaction input.
struct CreationTxInput;
/// Marker type under [`Bytecode`] indicating that the struct was obtained from the result of local compilation.
struct CompilationResult;

/// Wrapper under `evm.bytecode.object` from the standard output JSON
/// (https://docs.soliditylang.org/en/latest/using-the-compiler.html#output-description)
/// excluding metadata hash and optionally including constructor arguments used on a contract creation.
#[derive(PartialEq)]
struct Bytecode<Source> {
    /// Bytecode used in contract creation transaction excluding
    /// encoded metadata hash and following data
    bytecode: bytes::Bytes,
    /// Bytes used in contract creation transaction after
    /// encoded metadata hash
    /// (may include some hex data concatenated with constructor arguments)
    bytes_after_metadata_hash: bytes::Bytes,
    /// The marker indicating what type of data a struct is "tied" to
    source: std::marker::PhantomData<Source>,
}

impl<Source> Clone for Bytecode<Source> {
    fn clone(&self) -> Self {
        Self {
            bytecode: self.bytecode.clone(),
            bytes_after_metadata_hash: self.bytes_after_metadata_hash.clone(),
            source: self.source,
        }
    }
}

impl<Source> Debug for Bytecode<Source> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bytecode")
            .field("bytecode", &self.bytecode)
            .field("bytes_after_metadata_Hash", &self.bytes_after_metadata_hash)
            .field("source", &self.source)
            .finish()
    }
}

impl<Source> Bytecode<Source> {
    /// Initializes the structure from string and parsed deployed bytecode.
    /// It removes metadata hash from the provided string and extracts
    /// bytecode and arguments passed after metadata.
    ///
    /// Deployed bytecode is required to extract metadata hash from the string.
    pub fn from_str(
        s: &str,
        deployed_bytecode: &DeployedBytecode,
    ) -> Result<Self, InitializationError> {
        let bytes = DisplayBytes::from_str(s)
            .map_err(|_| InitializationError::InvalidCreationTxInput(s.to_string()))?
            .0;

        Bytecode::try_from_bytes(bytes, deployed_bytecode)
    }

    /// Initializes the structure from bytes string and parsed deployed bytecode.
    /// It removes metadata hash from the provided string and extracts
    /// bytecode and arguments passed after metadata.
    ///
    /// Deployed bytecode is required to extract metadata hash from the string.
    pub fn try_from_bytes(
        bytes: bytes::Bytes,
        deployed_bytecode: &DeployedBytecode,
    ) -> Result<Self, InitializationError> {
        let expected_metadata_hash = deployed_bytecode.encoded_metadata_hash_with_length();
        let metadata_hash_size = expected_metadata_hash.len();
        let metadata_hash_start_index = bytes
            .windows(metadata_hash_size)
            .enumerate()
            .rev()
            .find(|&(_, w)| w == expected_metadata_hash)
            .map(|(i, _)| i);

        if metadata_hash_start_index.is_none() {
            return Err(InitializationError::MetadataHashMismatch(
                Mismatch::expected(expected_metadata_hash.into()),
            ));
        }

        let start = metadata_hash_start_index.unwrap();
        let size = metadata_hash_size;

        let bytecode = bytes.slice(0..start);
        let bytes_after_metadata_hash = bytes.slice(start + size..bytes.len());

        Ok(Self {
            bytecode,
            bytes_after_metadata_hash,
            source: std::marker::PhantomData,
        })
    }
}

impl Bytecode<CreationTxInput> {
    /// Extract constructor arguments using the bytecode obtained as a result of local compilation.
    /// If there are no constructor arguments, returns `Ok(None)`, otherwise returns `Ok`
    /// with encoded constructor arguments. If the extraction fails, returns `Err`.
    pub fn constructor_args(
        &self,
        compiled_bytecode: &Bytecode<CompilationResult>,
    ) -> Result<Option<bytes::Bytes>, VerificationError> {
        if let Some(constructor_args) = self
            .bytes_after_metadata_hash
            .strip_prefix(compiled_bytecode.bytes_after_metadata_hash.as_ref())
        {
            if constructor_args.is_empty() {
                Ok(None)
            } else {
                Ok(Some(
                    self.bytes_after_metadata_hash.slice_ref(constructor_args),
                ))
            }
        } else {
            Err(VerificationError::ExtraDataMismatch(Mismatch::new(
                compiled_bytecode.bytes_after_metadata_hash.clone().into(),
                self.bytes_after_metadata_hash.clone().into(),
            )))
        }
    }

    /// Verifies that bytecode and extra data obtained from creation transaction input
    /// corresponds to the bytecode and extra data obtained from local compilation result.
    pub fn verify_bytecode_with_extra_data(
        &self,
        compiled_bytecode: &Bytecode<CompilationResult>,
    ) -> Result<(), VerificationError> {
        if self.bytecode != compiled_bytecode.bytecode {
            return Err(VerificationError::BytecodeMismatch(Mismatch::new(
                compiled_bytecode.bytecode.clone().into(),
                self.bytecode.clone().into(),
            )));
        }

        if !self
            .bytes_after_metadata_hash
            .starts_with(compiled_bytecode.bytes_after_metadata_hash.as_ref())
        {
            return Err(VerificationError::ExtraDataMismatch(Mismatch::new(
                compiled_bytecode.bytes_after_metadata_hash.clone().into(),
                self.bytes_after_metadata_hash.clone().into(),
            )));
        }

        Ok(())
    }
}

/// Verifier used in contract verification.
///
/// Contains input data provided by the requester that will
/// further be used in verification process.
#[derive(Clone, Debug)]
pub(crate) struct Verifier {
    /// Bytecode used on the contract creation transaction
    bc_creation_tx_input: Bytecode<CreationTxInput>,
    /// Bytecode stored in the chain and being used by EVM
    bc_deployed_bytecode: DeployedBytecode,
}

impl Verifier {
    /// Instantiates a new verifier instance with input data provided by the requester.
    ///
    /// Returns [`InitializationError`] inside [`Err`] if either `deployed_bytecode` or `creation_tx_input` are invalid.
    pub fn new(
        creation_tx_input: &str,
        deployed_bytecode: &str,
    ) -> Result<Self, InitializationError> {
        let deployed_bytecode = DeployedBytecode::from_str(deployed_bytecode)?;
        let bytecode = Bytecode::from_str(creation_tx_input, &deployed_bytecode)?;

        Ok(Self {
            bc_deployed_bytecode: deployed_bytecode,
            bc_creation_tx_input: bytecode,
        })
    }

    /// Verifies input data provided on initialization by comparing it
    /// with compiler output received when compiling source data locally.
    ///
    /// Iterates through all contracts received from local compilation and
    /// returns [`VerificationSuccess`] with file path and contract name
    /// of succeeded contract, if any. Otherwise, returns [`None`].
    pub fn verify(&self, output: CompilerOutput) -> Option<VerificationSuccess> {
        for (path, contracts) in output.contracts {
            for (name, contract) in contracts {
                // TODO: add logging in case if error is `VerificationError::InternalError`
                if let Ok((abi, constructor_args)) = self.compare(&contract) {
                    return Some(VerificationSuccess {
                        file_path: path,
                        contract_name: name,
                        abi,
                        constructor_args: constructor_args.map(DisplayBytes::from),
                    });
                }
            }
        }

        None
    }

    /// Compares the result of local contract compilation with data specified on initialization.
    ///
    /// On success returns a tuple where first argument is a contract ABI, and the second
    /// is constructor arguments passed on actual contract initialization.
    fn compare(
        &self,
        contract: &Contract,
    ) -> Result<(ethabi::Contract, Option<Bytes>), VerificationError> {
        let deployed_bytecode = {
            let bytes = contract
                .get_deployed_bytecode_bytes()
                .ok_or(VerificationError::MissedLibrary)?;
            DeployedBytecode::try_from(bytes.0.clone())
                .map_err(|err| VerificationError::InvalidDeployedBytecode(err.to_string()))?
        };
        let bytecode = {
            let bytes = contract
                .get_bytecode_bytes()
                .ok_or_else(|| VerificationError::InternalError("Missing bytecode bytes".into()))?;
            Bytecode::<CompilationResult>::try_from_bytes(bytes.0.clone(), &deployed_bytecode)
                .map_err(|err| {
                    VerificationError::InternalError(format!("Invalid bytecode bytes: {:?}", err))
                })?
        };
        let abi = contract
            .get_abi()
            .ok_or_else(|| VerificationError::InternalError("Missing abi".into()))?;

        self.check_metadata_hash_solc_versions(&deployed_bytecode)?;

        self.bc_creation_tx_input
            .verify_bytecode_with_extra_data(&bytecode)?;

        let constructor_args = self.extract_constructor_args(abi.constructor(), &bytecode)?;

        Ok((abi.into_owned(), constructor_args))
    }

    /// Checks that solc versions obtained from metadata hash correspond
    /// for provided deployed bytecode and deployed bytecode obtained
    /// as a result of local compilation.
    fn check_metadata_hash_solc_versions(
        &self,
        deployed_bytecode: &DeployedBytecode,
    ) -> Result<(), VerificationError> {
        let compiled_solc = &deployed_bytecode.metadata_hash().solc;
        let bc_solc = &self.bc_deployed_bytecode.metadata_hash().solc;
        if bc_solc != compiled_solc {
            let compiled_solc = compiled_solc
                .as_ref()
                .map(|b| DisplayBytes::from(b.clone()).to_string());
            let bc_solc = bc_solc
                .as_ref()
                .map(|b| DisplayBytes::from(b.clone()).to_string());
            return Err(VerificationError::CompilerVersionMismatch(Mismatch::new(
                compiled_solc,
                bc_solc,
            )));
        }
        Ok(())
    }

    /// Extracts constructor arguments from the creation transaction input specified on
    /// [`Verifier`] initialization.
    ///
    /// Returns `Err` if constructor arguments cannot be extracted (should not be the case
    /// if `Bytecode.verify_bytecode_with_extra_data` was called before).
    fn extract_constructor_args(
        &self,
        abi_constructor: Option<&Constructor>,
        bytecode: &Bytecode<CompilationResult>,
    ) -> Result<Option<Bytes>, VerificationError> {
        let encoded_constructor_args = self.bc_creation_tx_input.constructor_args(bytecode)?;

        let expects_constructor_args =
            abi_constructor.map(|input| input.inputs.len()).unwrap_or(0) > 0;

        match encoded_constructor_args {
            None if expects_constructor_args => Err(
                VerificationError::InvalidConstructorArguments(DisplayBytes::from([])),
            ),
            Some(encoded) if !expects_constructor_args => Err(
                VerificationError::InvalidConstructorArguments(encoded.into()),
            ),
            None => Ok(None),
            Some(encoded_constructor_args) => {
                let _constructor_args = self.parse_constructor_args(
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
        &self,
        encoded_args: Bytes,
        abi_constructor: &Constructor,
    ) -> Result<Vec<Token>, VerificationError> {
        let param_types = |inputs: &Vec<ethabi::Param>| -> Vec<ethabi::ParamType> {
            inputs.iter().map(|p| p.kind.clone()).collect()
        };
        let param_types = param_types(&abi_constructor.inputs);
        let tokens = ethabi::decode(&param_types, encoded_args.as_ref())
            .map_err(|_err| VerificationError::InvalidConstructorArguments(encoded_args.into()))?;

        Ok(tokens)
    }
}

#[cfg(test)]
mod verifier_initialization_tests {
    use super::*;
    use const_format::concatcp;

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
            InitializationError::MetadataHashMismatch(Mismatch::expected(
                DisplayBytes::from_str(DEFAULT_ENCODED_METADATA_HASH).unwrap()
            ))
        )
    }

    #[test]
    fn initialization_with_invalid_hex_as_creation_tx_input_should_fail() {
        let invalid_input = "0xabcdefghij";
        let verifier = Verifier::new(invalid_input, DEFAULT_DEPLOYED_BYTECODE);
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            InitializationError::InvalidCreationTxInput(invalid_input.to_string())
        )
    }

    #[test]
    fn initialization_with_empty_deployed_bytecode_should_fail() {
        let verifier = Verifier::new(DEFAULT_CREATION_TX_INPUT, "");
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            InitializationError::MetadataHashParse("length is not encoded".to_string())
        )
    }

    #[test]
    fn initialization_with_invalid_hex_as_deployed_bytecode_should_fail() {
        let invalid_input = "0xabcdefghij";
        let verifier = Verifier::new(DEFAULT_CREATION_TX_INPUT, invalid_input);
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            InitializationError::InvalidDeployedBytecode(invalid_input.to_string())
        )
    }

    #[test]
    fn initialization_with_metadata_hash_mismatch_should_fail() {
        // {"ipfs": h'1220EB23CE2C13EA8739368F952F6C6A4B1F0623D147D2A19B6D4D26A61AB03FCD3E', "solc": 0.8.0}
        let another_metadata_hash = "a2646970667358221220eb23ce2c13ea8739368f952f6c6a4b1f0623d147d2a19b6d4d26a61ab03fcd3e64736f6c63430008000033";
        let verifier = Verifier::new(
            &format!(
                "{}{}",
                DEFAULT_BYTECODE_WITHOUT_METADATA_HASH, another_metadata_hash
            ),
            DEFAULT_DEPLOYED_BYTECODE,
        );
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            InitializationError::MetadataHashMismatch(Mismatch::expected(
                DisplayBytes::from_str(DEFAULT_ENCODED_METADATA_HASH).unwrap()
            ))
        );
    }
}

#[cfg(test)]
mod metadata_hash_deserialization_tests {
    use super::*;

    fn is_valid_custom_error(
        error: minicbor::decode::Error,
        expected: ParseMetadataHashError,
    ) -> bool {
        if !error.is_custom() {
            return false;
        }

        // Unfortunately, current `minicbor::decode::Error` implementation
        // does not allow to retrieve insides out of custom error,
        // so the only way to ensure the valid error occurred is by string comparison.
        let parse_metadata_hash_error_to_string = |err: ParseMetadataHashError| match err {
            ParseMetadataHashError::NonExhausted => "NonExhausted",
            ParseMetadataHashError::InvalidSolcType(_) => "InvalidSolcType",
            ParseMetadataHashError::DuplicateKeys => "DuplicateKeys",
        };
        format!("{:?}", error).contains(parse_metadata_hash_error_to_string(expected))
    }

    #[test]
    fn deserialization_metadata_hash_without_solc_tag() {
        // given
        // { "bzzr0": b"d4fba422541feba2d648f6657d9354ec14ea9f5919b520abe0feb60981d7b17c" }
        let hex =
            "a165627a7a72305820d4fba422541feba2d648f6657d9354ec14ea9f5919b520abe0feb60981d7b17c";
        let encoded = DisplayBytes::from_str(hex).unwrap().0;
        let expected = MetadataHash { solc: None };

        // when
        let decoded =
            MetadataHash::from_cbor(encoded).expect("Error when decoding valid metadata hash");

        // then
        assert_eq!(expected, decoded, "Incorrectly decoded");
    }

    #[test]
    fn deserialization_metadata_hash_with_solc_as_version() {
        // given
        // { "ipfs": b"1220BCC988B1311237F2C00CCD0BFBD8B01D24DC18F720603B0DE93FE6327DF53625", "solc": b'00080e' }
        let hex = "a2646970667358221220bcc988b1311237f2c00ccd0bfbd8b01d24dc18f720603b0de93fe6327df5362564736f6c634300080e";
        let encoded = DisplayBytes::from_str(hex).unwrap().0;
        let expected = MetadataHash {
            solc: Some("\u{0}\u{8}\u{e}".as_bytes().into()),
        };

        // when
        let decoded =
            MetadataHash::from_cbor(encoded).expect("Error when decoding valid metadata hash");

        // then
        assert_eq!(expected, decoded, "Incorrectly decoded")
    }

    #[test]
    fn deserialization_metadata_hash_with_solc_as_string() {
        // given
        // {"ipfs": b'1220BA5AF27FE13BC83E671BD6981216D35DF49AB3AC923741B8948B277F93FBF732', "solc": "0.8.15-ci.2022.5.23+commit.21591531"}
        let hex = "a2646970667358221220ba5af27fe13bc83e671bd6981216d35df49ab3ac923741b8948b277f93fbf73264736f6c637823302e382e31352d63692e323032322e352e32332b636f6d6d69742e3231353931353331";
        let encoded = DisplayBytes::from_str(hex).unwrap().0;
        let expected = MetadataHash {
            solc: Some("0.8.15-ci.2022.5.23+commit.21591531".as_bytes().into()),
        };

        // when
        let decoded =
            MetadataHash::from_cbor(encoded).expect("Error when decoding valid metadata hash");

        // then
        assert_eq!(expected, decoded, "Incorrectly decoded")
    }

    #[test]
    fn deserialization_of_non_cbor_hex_should_fail() {
        // given
        let hex = "1234567890";
        let encoded = DisplayBytes::from_str(hex).unwrap().0;

        // when
        let decoded = MetadataHash::from_cbor(encoded);

        // then
        assert!(decoded.is_err(), "Deserialization should fail");
        assert!(
            decoded.unwrap_err().is_type_mismatch(),
            "Should fail with type mismatch"
        )
    }

    #[test]
    fn deserialization_of_non_map_should_fail() {
        // given
        // "solc"
        let hex = "64736f6c63";
        let encoded = DisplayBytes::from_str(hex).unwrap().0;

        // when
        let decoded = MetadataHash::from_cbor(encoded);

        // then
        assert!(decoded.is_err(), "Deserialization should fail");
        assert!(
            decoded.unwrap_err().is_type_mismatch(),
            "Should fail with type mismatch"
        )
    }

    #[test]
    fn deserialization_with_duplicated_solc_should_fail() {
        // given
        // { "solc": b'000400', "ipfs": b"1220BCC988B1311237F2C00CCD0BFBD8B01D24DC18F720603B0DE93FE6327DF53625", "solc": b'00080e' }
        let hex = "a364736f6c6343000400646970667358221220bcc988b1311237f2c00ccd0bfbd8b01d24dc18f720603b0de93fe6327df5362564736f6c634300080e";
        let encoded = DisplayBytes::from_str(hex).unwrap().0;

        // when
        let decoded = MetadataHash::from_cbor(encoded);

        // then
        assert!(decoded.is_err(), "Deserialization should fail");
        assert!(
            is_valid_custom_error(decoded.unwrap_err(), ParseMetadataHashError::DuplicateKeys),
            "Should fail with custom (DuplicateKey) error"
        );
    }

    #[test]
    fn deserialization_not_exhausted_should_fail() {
        // given
        // { "ipfs": b"1220BCC988B1311237F2C00CCD0BFBD8B01D24DC18F720603B0DE93FE6327DF53625", "solc": b'00080e' } \
        // { "bzzr0": b"d4fba422541feba2d648f6657d9354ec14ea9f5919b520abe0feb60981d7b17c" }
        let hex = format!(
            "{}{}",
            "a2646970667358221220bcc988b1311237f2c00ccd0bfbd8b01d24dc18f720603b0de93fe6327df5362564736f6c634300080e",
            "a165627a7a72305820d4fba422541feba2d648f6657d9354ec14ea9f5919b520abe0feb60981d7b17c"
        );
        let encoded = DisplayBytes::from_str(&hex).unwrap().0;

        // when
        let decoded = MetadataHash::from_cbor(encoded);

        // then
        assert!(decoded.is_err(), "Deserialization should fail");
        assert!(
            is_valid_custom_error(decoded.unwrap_err(), ParseMetadataHashError::NonExhausted),
            "Should fail with custom (NonExhausted) error"
        );
    }

    #[test]
    fn deserialization_with_not_enough_elements_should_fail() {
        // given
        // 3 elements expected in the map but got only 2:
        // { "ipfs": b"1220BCC988B1311237F2C00CCD0BFBD8B01D24DC18F720603B0DE93FE6327DF53625", "solc": b'00080e' }
        let hex = "a3646970667358221220bcc988b1311237f2c00ccd0bfbd8b01d24dc18f720603b0de93fe6327df5362564736f6c634300080e";
        let encoded = DisplayBytes::from_str(&hex).unwrap().0;

        // when
        let decoded = MetadataHash::from_cbor(encoded);

        // then
        assert!(decoded.is_err(), "Deserialization should fail");
        assert!(
            decoded.unwrap_err().is_end_of_input(),
            "Should fail with end of input error"
        );
    }

    #[test]
    fn deserialization_with_solc_neither_bytes_nor_string_should_fail() {
        // given
        // { "ipfs": b"1220BCC988B1311237F2C00CCD0BFBD8B01D24DC18F720603B0DE93FE6327DF53625", "solc": 123 } \
        let hex= "a2646970667358221220bcc988b1311237f2c00ccd0bfbd8b01d24dc18f720603b0de93fe6327df5362564736f6c63187B";
        let encoded = DisplayBytes::from_str(&hex).unwrap().0;

        // when
        let decoded = MetadataHash::from_cbor(encoded);

        // then
        assert!(decoded.is_err(), "Deserialization should fail");
        assert!(
            is_valid_custom_error(
                decoded.unwrap_err(),
                ParseMetadataHashError::InvalidSolcType(minicbor::data::Type::Int)
            ),
            "Should fail with custom (InvalidSolcType) error"
        );
    }
}
