use super::errors::{BytecodeInitError, VerificationErrorKind};
use bytes::{Buf, Bytes};
use ethers_solc::{
    artifacts::{Contract, Offsets},
    Artifact,
};
use mismatch::Mismatch;
use solidity_metadata::MetadataHash;
use std::{collections::BTreeMap, marker::PhantomData};

/// Types that can be used as Bytecode source indicator
pub trait Source {
    /// Performs conversion from [`Contract`] into valid bytecode
    fn try_bytes_from_contract(contract: &Contract) -> Result<Bytes, BytecodeInitError>;

    /// Indicates whether constructor arguments exist for the source
    /// (used when comparing unused bytes with constructor ABI)
    fn has_constructor_args() -> bool;

    fn has_immutable_references() -> bool;

    fn source_kind() -> SourceKind;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceKind {
    CreationTxInput,
    DeployedBytecode,
}

/// An indicator used in [`Bytecode`] showing that underlying bytecode
/// was obtained from on chain deployed bytecode.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeployedBytecode;

impl Source for DeployedBytecode {
    fn try_bytes_from_contract(contract: &Contract) -> Result<Bytes, BytecodeInitError> {
        let bytes = contract
            .get_deployed_bytecode_bytes()
            .ok_or_else(|| {
                let bytecode = contract
                    .get_deployed_bytecode_object()
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                BytecodeInitError::InvalidDeployedBytecode(bytecode)
            })?
            .0
            .clone();

        Ok(bytes)
    }

    fn has_constructor_args() -> bool {
        false
    }

    fn has_immutable_references() -> bool {
        true
    }

    fn source_kind() -> SourceKind {
        SourceKind::DeployedBytecode
    }
}

/// An indicator used in [`Bytecode`] showing that underlying bytecode
/// was obtained from on chain creation transaction input
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreationTxInput;

impl Source for CreationTxInput {
    fn try_bytes_from_contract(contract: &Contract) -> Result<Bytes, BytecodeInitError> {
        let bytes = contract
            .get_bytecode_bytes()
            .ok_or_else(|| {
                let bytecode = contract
                    .get_bytecode_object()
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                BytecodeInitError::InvalidCreationTxInput(bytecode)
            })?
            .0
            .clone();

        Ok(bytes)
    }

    fn has_constructor_args() -> bool {
        true
    }

    fn has_immutable_references() -> bool {
        false
    }

    fn source_kind() -> SourceKind {
        SourceKind::CreationTxInput
    }
}

/// An indicator used in [`Bytecode`] showing that underlying bytecode
/// was obtained from on chain creation transaction input but should not check constructor arguments.
///
/// Used for the verification of blueprint contracts, as constructor arguments
/// are inserted only during the actual create_from_blueprint calls.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreationTxInputWithoutConstructorArgs;

impl Source for CreationTxInputWithoutConstructorArgs {
    fn try_bytes_from_contract(contract: &Contract) -> Result<Bytes, BytecodeInitError> {
        let bytes = contract
            .get_bytecode_bytes()
            .ok_or_else(|| {
                let bytecode = contract
                    .get_bytecode_object()
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                BytecodeInitError::InvalidCreationTxInput(bytecode)
            })?
            .0
            .clone();

        Ok(bytes)
    }

    fn has_constructor_args() -> bool {
        false
    }

    fn has_immutable_references() -> bool {
        false
    }

    fn source_kind() -> SourceKind {
        SourceKind::CreationTxInput
    }
}

/// Encapsulate bytecode from specified source
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bytecode<T> {
    /// Raw bytecode bytes used in corresponding source
    bytecode: Bytes,
    /// Indicates the source of bytecode (DeployedBytecode, CreationTxInput)
    source: PhantomData<T>,
}

impl<T> Bytecode<T> {
    pub fn new(bytecode: Bytes) -> Result<Self, BytecodeInitError> {
        if bytecode.is_empty() {
            return Err(BytecodeInitError::Empty);
        }

        Ok(Self {
            bytecode,
            source: PhantomData,
        })
    }

    pub fn bytecode(&self) -> &Bytes {
        &self.bytecode
    }
}

impl<T: Source> TryFrom<&Contract> for Bytecode<T> {
    type Error = BytecodeInitError;

    fn try_from(contract: &Contract) -> Result<Self, Self::Error> {
        let bytes = T::try_bytes_from_contract(contract)?;
        Bytecode::new(bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BytecodePart {
    Main { raw: Bytes },
    Metadata { raw: Bytes, metadata: MetadataHash },
}

impl BytecodePart {
    pub fn size(&self) -> usize {
        match self {
            BytecodePart::Main { raw } => raw.len(),
            BytecodePart::Metadata { raw, .. } => raw.len(),
        }
    }
}

/// Encapsulates result of local source code compilation.
/// Splits compiled creation transaction input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalBytecode<T> {
    creation_tx_input: Bytecode<CreationTxInput>,
    deployed_bytecode: Bytecode<DeployedBytecode>,

    pub creation_tx_input_parts: Vec<BytecodePart>,
    pub deployed_bytecode_parts: Vec<BytecodePart>,

    pub immutable_references: BTreeMap<String, Vec<Offsets>>,

    source: PhantomData<T>,
}

impl<T> LocalBytecode<T> {
    /// Initializes a new [`LocalBytecode`] struct.
    ///
    /// `bytecode` and `bytecode_modified` must differ only in metadata hashes.
    ///
    /// Any error here is [`VerificationErrorKind::InternalError`], as both original
    /// and modified bytecodes are obtained as a result of local compilation.
    pub fn new(
        (creation_tx_input, deployed_bytecode): (
            Bytecode<CreationTxInput>,
            Bytecode<DeployedBytecode>,
        ),
        (creation_tx_input_modified, deployed_bytecode_modified): (
            Bytecode<CreationTxInput>,
            Bytecode<DeployedBytecode>,
        ),
        immutable_references: BTreeMap<String, Vec<Offsets>>,
    ) -> Result<Self, VerificationErrorKind> {
        let creation_tx_input_parts = split(
            creation_tx_input.bytecode(),
            creation_tx_input_modified.bytecode(),
        )
        .map_err(|err| VerificationErrorKind::InternalError(format!("{err:#}")))?;
        let deployed_bytecode_parts = split(
            deployed_bytecode.bytecode(),
            deployed_bytecode_modified.bytecode(),
        )
        .map_err(|err| VerificationErrorKind::InternalError(format!("{err:#}")))?;

        Ok(Self {
            creation_tx_input,
            deployed_bytecode,
            creation_tx_input_parts,
            deployed_bytecode_parts,
            immutable_references,

            source: Default::default(),
        })
    }

    pub fn bytecode(&self) -> &Bytes
    where
        T: Source,
    {
        match T::source_kind() {
            SourceKind::CreationTxInput => self.creation_tx_input.bytecode(),
            SourceKind::DeployedBytecode => self.deployed_bytecode.bytecode(),
        }
    }

    pub fn bytecode_parts(&self) -> &Vec<BytecodePart>
    where
        T: Source,
    {
        match T::source_kind() {
            SourceKind::CreationTxInput => &self.creation_tx_input_parts,
            SourceKind::DeployedBytecode => &self.deployed_bytecode_parts,
        }
    }
}

/// Splits bytecode onto [`BytecodePart`]s using bytecode with modified metadata hashes.
///
/// Any error here is [`VerificationErrorKind::InternalError`], as both original
/// and modified bytecodes are obtained as a result of local compilation.
pub fn split(raw: &Bytes, raw_modified: &Bytes) -> Result<Vec<BytecodePart>, anyhow::Error> {
    if raw.len() != raw_modified.len() {
        anyhow::bail!(
            "bytecode and modified bytecode length mismatch: {}",
            Mismatch::new(raw.len(), raw_modified.len())
        )
    }

    let parts_total_size =
        |parts: &Vec<BytecodePart>| -> usize { parts.iter().fold(0, |size, el| size + el.size()) };

    let mut result = Vec::new();

    let mut i = 0usize;
    while i < raw.len() {
        let decoded = parse_bytecode_parts(&raw.slice(i..raw.len()), &raw_modified[i..])?;
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
) -> Result<Vec<BytecodePart>, anyhow::Error> {
    let mut parts = Vec::new();

    let len = raw.len();

    // search for the first non-matching byte
    let mut index = raw
        .iter()
        .zip(raw_modified.iter())
        .position(|(a, b)| a != b);

    // There is some non-matching byte - part of the metadata part byte.
    if let Some(mut i) = index {
        let is_metadata_length_valid = |metadata_length: usize, i: usize| {
            if len < i + metadata_length + 2 {
                return false;
            }

            // Decode length of metadata hash representation
            let mut metadata_length_raw =
                raw.slice((i + metadata_length)..(i + metadata_length + 2));
            let encoded_metadata_length = metadata_length_raw.get_u16() as usize;
            if encoded_metadata_length != metadata_length {
                return false;
            }

            true
        };

        // `i` is the first different byte. The metadata hash itself started somewhere earlier
        // (at least for "a1"/"a2" indicating number of elements in cbor mapping).
        // Next steps are trying to find that beginning.

        let (metadata, metadata_length) = loop {
            let mut result = MetadataHash::from_cbor(&raw[i..]);
            while result.is_err() {
                // It is the beginning of the bytecode segment but no metadata hash has been parsed
                if i == 0 {
                    anyhow::bail!("failed to parse bytecode part",)
                }
                i -= 1;

                result = MetadataHash::from_cbor(&raw[i..]);
            }

            let (metadata, metadata_length) = result.unwrap();

            if is_metadata_length_valid(metadata_length, i) {
                break (metadata, metadata_length);
            }

            i -= 1;
        };

        parts.push(BytecodePart::Metadata {
            raw: raw.slice(i..(i + metadata_length + 2)),
            metadata,
        });

        // Update index to point where metadata part begins
        index = Some(i);
    }

    // If there is something before metadata part (if any)
    // belongs to main part
    let i = index.unwrap_or(len);
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

#[cfg(test)]
mod local_bytecode_initialization_tests {
    use super::*;
    use crate::DisplayBytes;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    const CREATION_TX_INPUT_MAIN_PART_1: &str = "608060405234801561001057600080fd5b506040518060200161002190610050565b6020820181038252601f19601f820116604052506000908051906020019061004a92919061005c565b5061015f565b605c806101ac83390190565b8280546100689061012e565b90600052602060002090601f01602090048101928261008a57600085556100d1565b82601f106100a357805160ff19168380011785556100d1565b828001600101855582156100d1579182015b828111156100d05782518255916020019190600101906100b5565b5b5090506100de91906100e2565b5090565b5b808211156100fb5760008160009055506001016100e3565b5090565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b6000600282049050600182168061014657607f821691505b602082108103610159576101586100ff565b5b50919050565b603f8061016d6000396000f3fe6080604052600080fdfe";
    const CREATION_TX_INPUT_MAIN_PART_2: &str =
        "6080604052348015600f57600080fd5b50603f80601d6000396000f3fe6080604052600080fdfe";

    const DEPLOYED_BYTECODE_MAIN_PART_1: &str = "6080604052600080fdfe";
    const DEPLOYED_BYTECODE_MAIN_PART_2: &str = "6080604052600080cafe";

    const METADATA_PART_1: &str = "a26469706673582212202e82fb6222f966f0e56dc49cd1fb8a6b5eac9bdf74f62b8a5e9d8812901095d664736f6c634300080e0033";
    const METADATA_PART_2: &str = "a2646970667358221220bd9f7fd5fb164e10dd86ccc9880d27a177e74ba873e6a9b97b6c4d7062b26ff064736f6c634300080e0033";

    const METADATA_PART1_MODIFIED: &str = "a264697066735822122028c67e368422bc9c0b12226a099aa62a1facd39b08a84427d7f3efe1e37029b864736f6c634300080e0033";
    const METADATA_PART2_MODIFIED: &str = "a26469706673582212206b331720b143820ca2e65d7db53a1b005672433fcb7f2da3ab539851bddc226a64736f6c634300080e0033";

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Bytecodes<T> {
        pub local_bytecode: LocalBytecode<T>,
        pub creation_tx_input: Bytecode<CreationTxInput>,
        pub deployed_bytecode: Bytecode<DeployedBytecode>,
    }

    fn new_bytecode<T: Source>(bytecode: &str) -> Result<Bytecode<T>, BytecodeInitError> {
        let bytecode = DisplayBytes::from_str(bytecode)
            .expect("Invalid bytecode")
            .0;
        Bytecode::new(bytecode)
    }

    fn new_local_bytecode<T: Source>(
        (creation_tx_input, deployed_bytecode): (&str, &str),
        (creation_tx_input_modified, deployed_bytecode_modified): (&str, &str),
    ) -> Result<Bytecodes<T>, VerificationErrorKind> {
        let creation_tx_input: Bytecode<CreationTxInput> =
            new_bytecode(creation_tx_input).expect("Bytecode initialization failed");
        let creation_tx_input_modified = new_bytecode(creation_tx_input_modified)
            .expect("Modified bytecode initialization failed");

        let deployed_bytecode: Bytecode<DeployedBytecode> =
            new_bytecode(deployed_bytecode).expect("Bytecode initialization failed");
        let deployed_bytecode_modified = new_bytecode(deployed_bytecode_modified)
            .expect("Modified bytecode initialization failed");

        LocalBytecode::new(
            (creation_tx_input.clone(), deployed_bytecode.clone()),
            (creation_tx_input_modified, deployed_bytecode_modified),
            Default::default(),
        )
        .map(|local_bytecode| Bytecodes {
            local_bytecode,
            creation_tx_input,
            deployed_bytecode,
        })
    }

    fn main_bytecode_part(raw: &str) -> BytecodePart {
        let raw = DisplayBytes::from_str(raw)
            .expect("Main bytecode part is invalid hex")
            .0;
        BytecodePart::Main { raw }
    }

    fn metadata_bytecode_part(raw: &str) -> BytecodePart {
        let raw = DisplayBytes::from_str(raw)
            .expect("Metadata bytecode part is invalid hex")
            .0;
        let (metadata, len) =
            MetadataHash::from_cbor(&raw).expect("Metadata bytecode part is not metadata");
        if raw.len() != len + 2 {
            panic!("Metadata bytecode part has invalid length");
        }
        BytecodePart::Metadata { raw, metadata }
    }

    #[test]
    fn without_metadata() {
        let creation_tx_input_str = CREATION_TX_INPUT_MAIN_PART_1.to_string();
        let creation_tx_input_modified_str = CREATION_TX_INPUT_MAIN_PART_1.to_string();

        let deployed_bytecode_str = DEPLOYED_BYTECODE_MAIN_PART_1.to_string();
        let deployed_bytecode_modified_str = DEPLOYED_BYTECODE_MAIN_PART_1.to_string();

        // Verify bytecode for creation tx input first
        {
            let Bytecodes {
                local_bytecode,
                creation_tx_input,
                ..
            }: Bytecodes<CreationTxInput> = new_local_bytecode(
                (&creation_tx_input_str, &deployed_bytecode_str),
                (
                    &creation_tx_input_modified_str,
                    &deployed_bytecode_modified_str,
                ),
            )
            .expect("Initialization of local bytecode failed");
            assert_eq!(
                creation_tx_input.bytecode(),
                local_bytecode.bytecode(),
                "Invalid bytecode"
            );
            assert_eq!(
                &vec![main_bytecode_part(CREATION_TX_INPUT_MAIN_PART_1)],
                local_bytecode.bytecode_parts(),
                "Invalid bytecode parts"
            );
        }

        // Verify bytecode for deployed bytecode next
        {
            let Bytecodes {
                local_bytecode,
                deployed_bytecode,
                ..
            }: Bytecodes<DeployedBytecode> = new_local_bytecode(
                (&creation_tx_input_str, &deployed_bytecode_str),
                (
                    &creation_tx_input_modified_str,
                    &deployed_bytecode_modified_str,
                ),
            )
            .expect("Initialization of local bytecode failed");
            assert_eq!(
                deployed_bytecode.bytecode(),
                local_bytecode.bytecode(),
                "Invalid bytecode"
            );
            assert_eq!(
                &vec![main_bytecode_part(DEPLOYED_BYTECODE_MAIN_PART_1)],
                local_bytecode.bytecode_parts(),
                "Invalid bytecode parts"
            );
        }
    }

    #[test]
    fn with_one_metadata() {
        let creation_tx_input_str = format!("{CREATION_TX_INPUT_MAIN_PART_1}{METADATA_PART_1}");
        let creation_tx_input_modified_str =
            format!("{CREATION_TX_INPUT_MAIN_PART_1}{METADATA_PART1_MODIFIED}");

        let deployed_bytecode_str = format!("{DEPLOYED_BYTECODE_MAIN_PART_1}{METADATA_PART_1}");
        let deployed_bytecode_modified_str =
            format!("{DEPLOYED_BYTECODE_MAIN_PART_1}{METADATA_PART1_MODIFIED}");

        // Verify bytecode for creation tx input first
        {
            let Bytecodes {
                local_bytecode,
                creation_tx_input,
                ..
            }: Bytecodes<CreationTxInput> = new_local_bytecode(
                (&creation_tx_input_str, &deployed_bytecode_str),
                (
                    &creation_tx_input_modified_str,
                    &deployed_bytecode_modified_str,
                ),
            )
            .expect("Initialization of local bytecode failed");
            assert_eq!(
                creation_tx_input.bytecode(),
                local_bytecode.bytecode(),
                "Invalid bytecode"
            );
            assert_eq!(
                &vec![
                    main_bytecode_part(CREATION_TX_INPUT_MAIN_PART_1),
                    metadata_bytecode_part(METADATA_PART_1),
                ],
                local_bytecode.bytecode_parts(),
                "Invalid bytecode parts"
            );
        }

        // Verify bytecode for deployed bytecode next
        {
            let Bytecodes {
                local_bytecode,
                deployed_bytecode,
                ..
            }: Bytecodes<DeployedBytecode> = new_local_bytecode(
                (&creation_tx_input_str, &deployed_bytecode_str),
                (
                    &creation_tx_input_modified_str,
                    &deployed_bytecode_modified_str,
                ),
            )
            .expect("Initialization of local bytecode failed");
            assert_eq!(
                deployed_bytecode.bytecode(),
                local_bytecode.bytecode(),
                "Invalid bytecode"
            );
            assert_eq!(
                &vec![
                    main_bytecode_part(DEPLOYED_BYTECODE_MAIN_PART_1),
                    metadata_bytecode_part(METADATA_PART_1),
                ],
                local_bytecode.bytecode_parts(),
                "Invalid bytecode parts"
            );
        }
    }

    #[test]
    fn with_two_metadata() {
        let creation_tx_input_str = format!(
            "{CREATION_TX_INPUT_MAIN_PART_1}{METADATA_PART_1}{CREATION_TX_INPUT_MAIN_PART_2}{METADATA_PART_2}"
        );
        let creation_tx_input_modified_str = format!(
            "{CREATION_TX_INPUT_MAIN_PART_1}{METADATA_PART1_MODIFIED}{CREATION_TX_INPUT_MAIN_PART_2}{METADATA_PART2_MODIFIED}"
        );

        let deployed_bytecode_str = format!(
            "{DEPLOYED_BYTECODE_MAIN_PART_1}{METADATA_PART_1}{DEPLOYED_BYTECODE_MAIN_PART_2}{METADATA_PART_2}"
        );
        let deployed_bytecode_modified_str = format!(
            "{DEPLOYED_BYTECODE_MAIN_PART_1}{METADATA_PART1_MODIFIED}{DEPLOYED_BYTECODE_MAIN_PART_2}{METADATA_PART2_MODIFIED}"
        );

        // Verify bytecode for creation tx input first
        {
            let Bytecodes {
                local_bytecode,
                creation_tx_input,
                ..
            }: Bytecodes<CreationTxInput> = new_local_bytecode(
                (&creation_tx_input_str, &deployed_bytecode_str),
                (
                    &creation_tx_input_modified_str,
                    &deployed_bytecode_modified_str,
                ),
            )
            .expect("Initialization of local bytecode failed");
            assert_eq!(
                creation_tx_input.bytecode(),
                local_bytecode.bytecode(),
                "Invalid bytecode"
            );
            assert_eq!(
                &vec![
                    main_bytecode_part(CREATION_TX_INPUT_MAIN_PART_1),
                    metadata_bytecode_part(METADATA_PART_1),
                    main_bytecode_part(CREATION_TX_INPUT_MAIN_PART_2),
                    metadata_bytecode_part(METADATA_PART_2),
                ],
                local_bytecode.bytecode_parts(),
                "Invalid bytecode parts"
            );
        }

        // Verify bytecode for deployed bytecode next
        {
            let Bytecodes {
                local_bytecode,
                deployed_bytecode,
                ..
            }: Bytecodes<DeployedBytecode> = new_local_bytecode(
                (&creation_tx_input_str, &deployed_bytecode_str),
                (
                    &creation_tx_input_modified_str,
                    &deployed_bytecode_modified_str,
                ),
            )
            .expect("Initialization of local bytecode failed");
            assert_eq!(
                deployed_bytecode.bytecode(),
                local_bytecode.bytecode(),
                "Invalid bytecode"
            );
            assert_eq!(
                &vec![
                    main_bytecode_part(DEPLOYED_BYTECODE_MAIN_PART_1),
                    metadata_bytecode_part(METADATA_PART_1),
                    main_bytecode_part(DEPLOYED_BYTECODE_MAIN_PART_2),
                    metadata_bytecode_part(METADATA_PART_2),
                ],
                local_bytecode.bytecode_parts(),
                "Invalid bytecode parts"
            );
        }
    }

    #[test]
    fn with_two_metadata_but_one_main_part() {
        let creation_tx_input_str =
            format!("{CREATION_TX_INPUT_MAIN_PART_1}{METADATA_PART_1}{METADATA_PART_2}");
        let creation_tx_input_modified_str = format!(
            "{CREATION_TX_INPUT_MAIN_PART_1}{METADATA_PART1_MODIFIED}{METADATA_PART2_MODIFIED}"
        );

        let deployed_bytecode_str =
            format!("{DEPLOYED_BYTECODE_MAIN_PART_1}{METADATA_PART_1}{METADATA_PART_2}");
        let deployed_bytecode_modified_str = format!(
            "{DEPLOYED_BYTECODE_MAIN_PART_1}{METADATA_PART1_MODIFIED}{METADATA_PART2_MODIFIED}"
        );

        // Verify bytecode for creation tx input first
        {
            let Bytecodes {
                local_bytecode,
                creation_tx_input,
                ..
            }: Bytecodes<CreationTxInput> = new_local_bytecode(
                (&creation_tx_input_str, &deployed_bytecode_str),
                (
                    &creation_tx_input_modified_str,
                    &deployed_bytecode_modified_str,
                ),
            )
            .expect("Initialization of local bytecode failed");
            assert_eq!(
                creation_tx_input.bytecode(),
                local_bytecode.bytecode(),
                "Invalid bytecode"
            );
            assert_eq!(
                &vec![
                    main_bytecode_part(CREATION_TX_INPUT_MAIN_PART_1),
                    metadata_bytecode_part(METADATA_PART_1),
                    metadata_bytecode_part(METADATA_PART_2),
                ],
                local_bytecode.bytecode_parts(),
                "Invalid bytecode parts"
            );
        }

        // Verify bytecode for deployed bytecode next
        {
            let Bytecodes {
                local_bytecode,
                deployed_bytecode,
                ..
            }: Bytecodes<DeployedBytecode> = new_local_bytecode(
                (&creation_tx_input_str, &deployed_bytecode_str),
                (
                    &creation_tx_input_modified_str,
                    &deployed_bytecode_modified_str,
                ),
            )
            .expect("Initialization of local bytecode failed");
            assert_eq!(
                deployed_bytecode.bytecode(),
                local_bytecode.bytecode(),
                "Invalid bytecode"
            );
            assert_eq!(
                &vec![
                    main_bytecode_part(DEPLOYED_BYTECODE_MAIN_PART_1),
                    metadata_bytecode_part(METADATA_PART_1),
                    metadata_bytecode_part(METADATA_PART_2),
                ],
                local_bytecode.bytecode_parts(),
                "Invalid bytecode parts"
            );
        }
    }

    #[test]
    fn with_different_lengths_should_fail() {
        let creation_tx_input_str = format!("{CREATION_TX_INPUT_MAIN_PART_1}{METADATA_PART_1}");
        // additional byte
        let creation_tx_input_modified_str =
            format!("{CREATION_TX_INPUT_MAIN_PART_1}{METADATA_PART1_MODIFIED}12");

        let deployed_bytecode_str = DEPLOYED_BYTECODE_MAIN_PART_1.to_string();
        let deployed_bytecode_modified_str = DEPLOYED_BYTECODE_MAIN_PART_1.to_string();

        let local_bytecode: Result<Bytecodes<CreationTxInput>, _> = new_local_bytecode(
            (&creation_tx_input_str, &deployed_bytecode_str),
            (
                &creation_tx_input_modified_str,
                &deployed_bytecode_modified_str,
            ),
        );

        assert!(
            local_bytecode.is_err(),
            "Should fail, but: {:?}",
            local_bytecode.unwrap()
        );
        match local_bytecode.unwrap_err() {
            VerificationErrorKind::InternalError(error) => {
                assert!(
                    error.contains("length mismatch"),
                    "Invalid error message: {error}"
                )
            }
            _ => panic!("Invalid error"),
        }
    }

    #[test]
    fn with_invalid_metadata_should_fail() {
        let creation_tx_input_str = format!("{CREATION_TX_INPUT_MAIN_PART_1}cafe{METADATA_PART_1}");
        let creation_tx_input_modified_str =
            format!("{CREATION_TX_INPUT_MAIN_PART_1}abcd{METADATA_PART1_MODIFIED}");

        let deployed_bytecode_str = DEPLOYED_BYTECODE_MAIN_PART_1.to_string();
        let deployed_bytecode_modified_str = DEPLOYED_BYTECODE_MAIN_PART_1.to_string();

        let local_bytecode: Result<Bytecodes<CreationTxInput>, _> = new_local_bytecode(
            (&creation_tx_input_str, &deployed_bytecode_str),
            (
                &creation_tx_input_modified_str,
                &deployed_bytecode_modified_str,
            ),
        );

        assert!(
            local_bytecode.is_err(),
            "Should fail, but: {:?}",
            local_bytecode.unwrap()
        );
        match local_bytecode.unwrap_err() {
            VerificationErrorKind::InternalError(error) => {
                assert!(
                    error.contains("failed to parse bytecode part"),
                    "Invalid error message: {error}"
                )
            }
            _ => panic!("Invalid error"),
        }
    }

    #[test]
    fn with_absent_metadata_length_should_fail() {
        let creation_tx_input_str = format!(
            "{}{}",
            CREATION_TX_INPUT_MAIN_PART_1,
            &METADATA_PART_1[..METADATA_PART_1.len() - 2]
        );
        let creation_tx_input_modified_str = format!(
            "{}{}",
            CREATION_TX_INPUT_MAIN_PART_1,
            &METADATA_PART1_MODIFIED[..METADATA_PART1_MODIFIED.len() - 2]
        );

        let deployed_bytecode_str = DEPLOYED_BYTECODE_MAIN_PART_1.to_string();
        let deployed_bytecode_modified_str = DEPLOYED_BYTECODE_MAIN_PART_1.to_string();

        let local_bytecode: Result<Bytecodes<CreationTxInput>, _> = new_local_bytecode(
            (&creation_tx_input_str, &deployed_bytecode_str),
            (
                &creation_tx_input_modified_str,
                &deployed_bytecode_modified_str,
            ),
        );

        assert!(
            local_bytecode.is_err(),
            "Should fail, but: {:?}",
            local_bytecode.unwrap()
        );
        match local_bytecode.unwrap_err() {
            VerificationErrorKind::InternalError(error) => {
                assert!(error.contains(""), "Invalid error message: {error}")
            }
            _ => panic!("failed to parse metadata length"),
        }
    }

    #[test]
    fn with_invalid_metadata_length_should_fail() {
        let creation_tx_input_str = format!(
            "{}{}{}",
            CREATION_TX_INPUT_MAIN_PART_1,
            &METADATA_PART_1[..METADATA_PART_1.len() - 4],
            "0031"
        );
        let creation_tx_input_modified_str = format!(
            "{}{}{}",
            CREATION_TX_INPUT_MAIN_PART_1,
            &METADATA_PART1_MODIFIED[..METADATA_PART1_MODIFIED.len() - 4],
            "0031"
        );

        let deployed_bytecode_str = DEPLOYED_BYTECODE_MAIN_PART_1.to_string();
        let deployed_bytecode_modified_str = DEPLOYED_BYTECODE_MAIN_PART_1.to_string();

        let local_bytecode: Result<Bytecodes<CreationTxInput>, _> = new_local_bytecode(
            (&creation_tx_input_str, &deployed_bytecode_str),
            (
                &creation_tx_input_modified_str,
                &deployed_bytecode_modified_str,
            ),
        );

        assert!(
            local_bytecode.is_err(),
            "Should fail, but: {:?}",
            local_bytecode.unwrap()
        );
        match local_bytecode.unwrap_err() {
            VerificationErrorKind::InternalError(error) => {
                assert!(
                    error.contains("failed to parse bytecode part"),
                    "Invalid error message: {error}"
                )
            }
            _ => panic!("failed to parse metadata length"),
        }
    }

    #[test]
    fn first_different_byte_is_valid_empty_map() {
        let creation_tx_input_str = "0x60556023600b82828239805160001a607314601657fe5b30600052607381538281f3fe73000000000000000000000000000000000000000030146080604052600080fdfea265627a7a72315820a05da1b258d199a6f4a643b2f9b479cb306c26c244caded90a94d976be7414d564736f6c63430006090032";
        let creation_tx_input_modified_str =
            "0x60556023600b82828239805160001a607314601657fe5b30600052607381538281f3fe73000000000000000000000000000000000000000030146080604052600080fdfea265627a7a723158207db4a5b2d2f40cb6d26365a00db0bb1c088df136323cb18502340fa1f41121ae64736f6c63430006090032";

        let deployed_bytecode_str = "0x60556023600b82828239805160001a607314601657fe5b30600052607381538281f3fe73000000000000000000000000000000000000000030146080604052600080fdfea265627a7a72315820a05da1b258d199a6f4a643b2f9b479cb306c26c244caded90a94d976be7414d564736f6c63430006090032";
        let deployed_bytecode_modified_str =
            "0x60556023600b82828239805160001a607314601657fe5b30600052607381538281f3fe73000000000000000000000000000000000000000030146080604052600080fdfea265627a7a723158207db4a5b2d2f40cb6d26365a00db0bb1c088df136323cb18502340fa1f41121ae64736f6c63430006090032";

        let Bytecodes {
            local_bytecode,
            creation_tx_input,
            ..
        }: Bytecodes<CreationTxInput> = new_local_bytecode(
            (creation_tx_input_str, deployed_bytecode_str),
            (
                creation_tx_input_modified_str,
                deployed_bytecode_modified_str,
            ),
        )
        .expect("Initialization of local bytecode failed");
        assert_eq!(
            creation_tx_input.bytecode(),
            local_bytecode.bytecode(),
            "Invalid bytecode"
        );
    }
}
