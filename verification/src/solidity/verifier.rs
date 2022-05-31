#![allow(dead_code, unused)]

use crate::types::Mismatch;
use bytes::Buf;
use ethers_core::types::{Bytes, ParseBytesError};
use ethers_solc::CompilerOutput;
use minicbor::{
    data::{Tag, Type},
    Decode, Decoder,
};
use std::{error::Error, str::FromStr};
use thiserror::Error;

/// Errors that may occur during initial [`Verifier`] setup
/// with input data provided by the requester.
#[derive(Clone, Debug, PartialEq, Error)]
pub(crate) enum InitializationError {
    #[error("creation transaction input is not a valid hex string")]
    InvalidCreationTxInput,
    #[error("deployed bytecode is not a valid hex string")]
    InvalidDeployedBytecode,
    #[error("cannot parse metadata hash from deployed bytecode: {0}")]
    MetadataHashParseError(String),
    #[error("creation transaction input has different metadata hash to deployed bytecode. {0}")]
    MetadataHashMismatch(Mismatch<Bytes>),
}

/// Errors that may occur during bytecode comparison step.
#[derive(Clone, Debug, Error)]
pub(crate) enum VerificationError {}

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
        for num in 0..number_of_elements {
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
        if let Ok(_) = d.datatype() {
            return Err(Error::custom(ParseMetadataHashError::NonExhausted));
        }

        let solc = solc.map(|v| bytes::Bytes::copy_from_slice(v));
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
    pub fn bytecode(&self) -> Bytes {
        self.bytecode.clone().into()
    }

    /// Returns a metadata hash
    pub fn metadata_hash(&self) -> &MetadataHash {
        &self.metadata_hash
    }

    /// Returns metadata hash encoded as bytes and concatenated with 2 bytes representing its length
    pub fn encoded_metadata_hash_with_length(&self) -> Bytes {
        let start = self.bytecode.len();
        let end = self.bytes.len();
        self.bytes.slice(start..end).into()
    }
}

impl FromStr for DeployedBytecode {
    type Err = InitializationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = Bytes::from_str(s)
            .map_err(|_| InitializationError::InvalidDeployedBytecode)?
            .0;

        DeployedBytecode::try_from(bytes)
    }
}

impl TryFrom<bytes::Bytes> for DeployedBytecode {
    type Error = InitializationError;

    fn try_from(encoded: bytes::Bytes) -> Result<Self, Self::Error> {
        // If metadata is present, last two bytes encode its length in a two-byte big-endian encoding
        if encoded.len() < 2 {
            return Err(InitializationError::MetadataHashParseError(
                "length is not encoded".to_string(),
            ));
        }

        // Further we will cut bytes from encoded representation, but original raw
        // bytes are still required for `DeployedBytecode.bytes`
        // (cloning takes O(1) due to internal `bytes::Bytes` implementation)
        let mut b = encoded.clone();

        // Decode length of metadata hash representation
        let metadata_hash_length = {
            let b_len = b.len();
            let mut length_bytes = b.split_off(b_len - 2);
            length_bytes.get_u16() as usize
        };

        if b.len() < metadata_hash_length {
            return Err(InitializationError::MetadataHashParseError(
                "not enough bytes".to_string(),
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
            return Err(InitializationError::MetadataHashParseError(message));
        }

        Ok(Self {
            bytecode: b,
            metadata_hash: metadata_hash.unwrap(),
            bytes: encoded,
        })
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
    /// (useful if multiple files contain contract with `contract_name`)
    file_path: Option<String>,
    /// Bytecode used on the contract creation transaction
    bc_creation_tx_input: BytecodeWithConstructorArgs,
    /// Bytecode stored in the chain and being used by EVMrap_err()
    bc_deployed_bytecode: DeployedBytecode,
}

impl Verifier {
    /// Instantiates a new verifier instance with input data provided by the requester.
    ///
    /// Returns [`InitializationError`] inside [`Err`] if either `deployed_bytecode` or `creation_tx_input` are invalid.
    pub fn new(
        contract_name: String,
        file_path: Option<String>,
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

#[cfg(test)]
mod verifier_initialization_tests {
    use super::*;
    use const_format::concatcp;

    const DEFAULT_CONTRACT_NAME: &'static str = "Contract";

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
    #[should_panic] // TODO: remove when implemented
    fn test_initialization_with_valid_data() {
        let verifier = Verifier::new(
            DEFAULT_CONTRACT_NAME.to_string(),
            None,
            DEFAULT_CREATION_TX_INPUT,
            DEFAULT_DEPLOYED_BYTECODE,
        );
        assert!(verifier.is_ok(), "Initialization with \"0x\" prefix failed");

        let verifier = Verifier::new(
            DEFAULT_CONTRACT_NAME.to_string(),
            None,
            &concatcp!("0x", DEFAULT_CREATION_TX_INPUT),
            &concatcp!("0x", DEFAULT_DEPLOYED_BYTECODE),
        );
        assert!(
            verifier.is_ok(),
            "Initialization without \"0x\" prefix failed"
        );
    }

    #[test]
    #[should_panic] // TODO: remove when implemented
    fn test_initialization_with_empty_creation_tx_input_should_fail() {
        let verifier = Verifier::new(
            DEFAULT_CONTRACT_NAME.to_string(),
            None,
            "",
            DEFAULT_DEPLOYED_BYTECODE,
        );
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            InitializationError::InvalidCreationTxInput
        )
    }

    #[test]
    #[should_panic] // TODO: remove when implemented
    fn test_initialization_with_creation_tx_input_as_invalid_hex_should_fail() {
        let invalid_input = "0xabcdefghij";
        let verifier = Verifier::new(
            DEFAULT_CONTRACT_NAME.to_string(),
            None,
            invalid_input,
            DEFAULT_DEPLOYED_BYTECODE,
        );
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            InitializationError::InvalidCreationTxInput
        )
    }

    #[test]
    #[should_panic] // TODO: remove when implemented
    fn test_initialization_with_empty_deployed_bytecode_should_fail() {
        let verifier = Verifier::new(
            DEFAULT_CONTRACT_NAME.to_string(),
            None,
            DEFAULT_CREATION_TX_INPUT,
            "",
        );
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            InitializationError::InvalidDeployedBytecode
        )
    }

    #[test]
    #[should_panic] // TODO: remove when implemented
    fn test_initialization_with_deployed_bytecode_as_invalid_hex_should_fail() {
        let invalid_input = "0xabcdefghij";
        let verifier = Verifier::new(
            DEFAULT_CONTRACT_NAME.to_string(),
            None,
            DEFAULT_CREATION_TX_INPUT,
            invalid_input,
        );
        assert!(verifier.is_err(), "Verifier initialization should fail");
        assert_eq!(
            verifier.unwrap_err(),
            InitializationError::InvalidDeployedBytecode
        )
    }

    #[test]
    #[should_panic] // TODO: remove when implemented
    fn test_initialization_with_metadata_hash_mismatch_should_fail() {
        // {"ipfs": h'1220EB23CE2C13EA8739368F952F6C6A4B1F0623D147D2A19B6D4D26A61AB03FCD3E', "solc": 0.8.0}
        let another_metadata_hash = "a2646970667358221220eb23ce2c13ea8739368f952f6c6a4b1f0623d147d2a19b6d4d26a61ab03fcd3e64736f6c63430008000033";
        let verifier = Verifier::new(
            DEFAULT_CONTRACT_NAME.to_string(),
            None,
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
                Bytes::from_str(DEFAULT_ENCODED_METADATA_HASH).unwrap()
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
    fn test_deserialization_metadata_hash_without_solc_tag() {
        // given
        // { "bzzr0": b"d4fba422541feba2d648f6657d9354ec14ea9f5919b520abe0feb60981d7b17c" }
        let hex =
            "a165627a7a72305820d4fba422541feba2d648f6657d9354ec14ea9f5919b520abe0feb60981d7b17c";
        let encoded = Bytes::from_str(hex).unwrap().0;
        let expected = MetadataHash { solc: None };

        // when
        let decoded =
            MetadataHash::from_cbor(encoded).expect("Error when decoding valid metadata hash");

        // then
        assert_eq!(expected, decoded, "Incorrectly decoded");
    }

    #[test]
    fn test_deserialization_metadata_hash_with_solc_as_version() {
        // given
        // { "ipfs": b"1220BCC988B1311237F2C00CCD0BFBD8B01D24DC18F720603B0DE93FE6327DF53625", "solc": b'00080e' }
        let hex = "a2646970667358221220bcc988b1311237f2c00ccd0bfbd8b01d24dc18f720603b0de93fe6327df5362564736f6c634300080e";
        let encoded = Bytes::from_str(hex).unwrap().0;
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
    fn test_deserialization_metadata_hash_with_solc_as_string() {
        // given
        // {"ipfs": b'1220BA5AF27FE13BC83E671BD6981216D35DF49AB3AC923741B8948B277F93FBF732', "solc": "0.8.15-ci.2022.5.23+commit.21591531"}
        let hex = "a2646970667358221220ba5af27fe13bc83e671bd6981216d35df49ab3ac923741b8948b277f93fbf73264736f6c637823302e382e31352d63692e323032322e352e32332b636f6d6d69742e3231353931353331";
        let encoded = Bytes::from_str(hex).unwrap().0;
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
    fn test_deserialization_of_non_cbor_hex_should_fail() {
        // given
        let hex = "1234567890";
        let encoded = Bytes::from_str(hex).unwrap().0;

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
    fn test_deserialization_of_non_map_should_fail() {
        // given
        // "solc"
        let hex = "64736f6c63";
        let encoded = Bytes::from_str(hex).unwrap().0;

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
    fn test_deserialization_with_duplicated_solc_should_fail() {
        // given
        // { "solc": b'000400', "ipfs": b"1220BCC988B1311237F2C00CCD0BFBD8B01D24DC18F720603B0DE93FE6327DF53625", "solc": b'00080e' }
        let hex = "a364736f6c6343000400646970667358221220bcc988b1311237f2c00ccd0bfbd8b01d24dc18f720603b0de93fe6327df5362564736f6c634300080e";
        let encoded = Bytes::from_str(hex).unwrap().0;

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
    fn test_deserialization_not_exhausted_should_fail() {
        // given
        // { "ipfs": b"1220BCC988B1311237F2C00CCD0BFBD8B01D24DC18F720603B0DE93FE6327DF53625", "solc": b'00080e' } \
        // { "bzzr0": b"d4fba422541feba2d648f6657d9354ec14ea9f5919b520abe0feb60981d7b17c" }
        let hex = format!(
            "{}{}",
            "a2646970667358221220bcc988b1311237f2c00ccd0bfbd8b01d24dc18f720603b0de93fe6327df5362564736f6c634300080e",
            "a165627a7a72305820d4fba422541feba2d648f6657d9354ec14ea9f5919b520abe0feb60981d7b17c"
        );
        let encoded = Bytes::from_str(&hex).unwrap().0;

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
    fn test_deserialization_with_not_enough_elements_should_fail() {
        // given
        // 3 elements expected in the map but got only 2:
        // { "ipfs": b"1220BCC988B1311237F2C00CCD0BFBD8B01D24DC18F720603B0DE93FE6327DF53625", "solc": b'00080e' }
        let hex = "a3646970667358221220bcc988b1311237f2c00ccd0bfbd8b01d24dc18f720603b0de93fe6327df5362564736f6c634300080e";
        let encoded = Bytes::from_str(&hex).unwrap().0;

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
    fn test_deserialization_with_solc_neither_bytes_nor_string_should_fail() {
        // given
        // { "ipfs": b"1220BCC988B1311237F2C00CCD0BFBD8B01D24DC18F720603B0DE93FE6327DF53625", "solc": 123 } \
        let hex= "a2646970667358221220bcc988b1311237f2c00ccd0bfbd8b01d24dc18f720603b0de93fe6327df5362564736f6c63187B";
        let encoded = Bytes::from_str(&hex).unwrap().0;

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
