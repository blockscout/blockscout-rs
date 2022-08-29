use super::{
    errors::{BytecodeInitializationError, VerificationErrorKind},
    metadata::MetadataHash,
};
use crate::{types::Mismatch, DisplayBytes};
use bytes::{Buf, Bytes};
use ethers_solc::{artifacts::Contract, Artifact};
use std::str::FromStr;

/// Combine creation_tx_input and deployed_bytecode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bytecode {
    /// Raw bytecode bytes used in contract creation transaction
    creation_tx_input: Bytes,
    /// Raw deployed bytecode bytes
    deployed_bytecode: Bytes,
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

        Self::from_bytes(creation_tx_input, deployed_bytecode)
    }

    pub fn from_bytes(
        creation_tx_input: Bytes,
        deployed_bytecode: Bytes,
    ) -> Result<Self, BytecodeInitializationError> {
        if creation_tx_input.is_empty() {
            return Err(BytecodeInitializationError::EmptyCreationTxInput);
        }

        if deployed_bytecode.is_empty() {
            return Err(BytecodeInitializationError::EmptyDeployedBytecode);
        }

        Ok(Self {
            creation_tx_input,
            deployed_bytecode,
        })
    }

    pub fn creation_tx_input(&self) -> &Bytes {
        &self.creation_tx_input
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
pub enum BytecodePart {
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

/// Encapsulates result of local source code compilation.
/// Splits compiled creation transaction input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalBytecode {
    bytecode: Bytecode,
    creation_tx_input_parts: Vec<BytecodePart>,
}

impl LocalBytecode {
    /// Initializes a new [`LocalBytecode`] struct.
    ///
    /// `bytecode` and `bytecode_modified` must differ only in metadata hashes.
    ///
    /// Any error here is [`VerificationErrorKind::InternalError`], as both original
    /// and modified bytecodes are obtained as a result of local compilation.
    pub fn new(
        bytecode: Bytecode,
        bytecode_modified: Bytecode,
    ) -> Result<Self, VerificationErrorKind> {
        let creation_tx_input_parts = Self::split(
            &bytecode.creation_tx_input,
            &bytecode_modified.creation_tx_input,
        )?;

        Ok(Self {
            bytecode,
            creation_tx_input_parts,
        })
    }

    pub fn creation_tx_input(&self) -> &Bytes {
        &self.bytecode.creation_tx_input
    }

    pub fn creation_tx_input_parts(&self) -> &Vec<BytecodePart> {
        &self.creation_tx_input_parts
    }

    /// Splits bytecode onto [`BytecodePart`]s using bytecode with modified metadata hashes.
    ///
    /// Any error here is [`VerificationErrorKind::InternalError`], as both original
    /// and modified bytecodes are obtained as a result of local compilation.
    fn split(
        raw: &Bytes,
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
        while i < raw.len() {
            let decoded = Self::parse_bytecode_parts(&raw.slice(i..raw.len()), &raw_modified[i..])?;
            let decoded_size = parts_total_size(&decoded);
            result.extend(decoded);

            i += decoded_size;
        }

        Ok(result)
    }

    /// Finds the next [`BytecodePart`]s into a series of bytes.
    ///
    /// Parses at most one [`BytecodePart::Main`] and one [`BytecodePart::Metadata`].
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
                return Err(VerificationErrorKind::InternalError(format!(
                    "encoded metadata length does not correspond to actual metadata length: {}",
                    Mismatch::new(metadata_length, encoded_metadata_length)
                )));
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
