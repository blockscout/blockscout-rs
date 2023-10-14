use super::smart_contract_verifier;
use anyhow::Context;
use entity::sea_orm_active_enums;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/********** Bytecode Part **********/

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BytecodePart {
    Main { data: Vec<u8> },
    Meta { data: Vec<u8> },
}

impl TryFrom<smart_contract_verifier::BytecodePart> for BytecodePart {
    type Error = anyhow::Error;

    fn try_from(value: smart_contract_verifier::BytecodePart) -> Result<Self, Self::Error> {
        let data = hex::decode(value.data.trim_start_matches("0x"))?;
        match value.r#type.as_str() {
            "main" => Ok(Self::Main { data }),
            "meta" => Ok(Self::Meta { data }),
            _ => Err(anyhow::anyhow!("Unknown type")),
        }
    }
}

impl From<&BytecodePart> for sea_orm_active_enums::PartType {
    fn from(value: &BytecodePart) -> Self {
        match value {
            BytecodePart::Main { .. } => Self::Main,
            BytecodePart::Meta { .. } => Self::Metadata,
        }
    }
}

impl BytecodePart {
    pub fn data(&self) -> &[u8] {
        match self {
            BytecodePart::Main { data } => data.as_ref(),
            BytecodePart::Meta { data } => data.as_ref(),
        }
    }

    pub fn data_owned(self) -> Vec<u8> {
        match self {
            BytecodePart::Main { data } => data,
            BytecodePart::Meta { data } => data,
        }
    }
}

/********** Bytecode Type **********/

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BytecodeType {
    CreationInput,
    DeployedBytecode,
}

impl From<BytecodeType> for sea_orm_active_enums::BytecodeType {
    fn from(value: BytecodeType) -> Self {
        match value {
            BytecodeType::CreationInput => sea_orm_active_enums::BytecodeType::CreationInput,
            BytecodeType::DeployedBytecode => sea_orm_active_enums::BytecodeType::DeployedBytecode,
        }
    }
}

impl From<BytecodeType> for smart_contract_verifier::BytecodeType {
    fn from(value: BytecodeType) -> Self {
        match value {
            BytecodeType::CreationInput => smart_contract_verifier::BytecodeType::CreationInput,
            BytecodeType::DeployedBytecode => {
                smart_contract_verifier::BytecodeType::DeployedBytecode
            }
        }
    }
}

/********** Source Type **********/

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    Solidity,
    Vyper,
    Yul,
}

impl From<SourceType> for sea_orm_active_enums::SourceType {
    fn from(source_type: SourceType) -> Self {
        match source_type {
            SourceType::Solidity => sea_orm_active_enums::SourceType::Solidity,
            SourceType::Vyper => sea_orm_active_enums::SourceType::Vyper,
            SourceType::Yul => sea_orm_active_enums::SourceType::Yul,
        }
    }
}

impl From<sea_orm_active_enums::SourceType> for SourceType {
    fn from(source_type: sea_orm_active_enums::SourceType) -> Self {
        match source_type {
            sea_orm_active_enums::SourceType::Solidity => SourceType::Solidity,
            sea_orm_active_enums::SourceType::Vyper => SourceType::Vyper,
            sea_orm_active_enums::SourceType::Yul => SourceType::Yul,
        }
    }
}

impl TryFrom<smart_contract_verifier::SourceType> for SourceType {
    type Error = anyhow::Error;

    fn try_from(value: smart_contract_verifier::SourceType) -> Result<Self, Self::Error> {
        match value {
            smart_contract_verifier::SourceType::Unspecified => {
                Err(anyhow::anyhow!("Unknown type: {}", value.as_str_name()))
            }
            smart_contract_verifier::SourceType::Solidity => Ok(SourceType::Solidity),
            smart_contract_verifier::SourceType::Vyper => Ok(SourceType::Vyper),
            smart_contract_verifier::SourceType::Yul => Ok(SourceType::Yul),
        }
    }
}

impl From<SourceType> for smart_contract_verifier::SourceType {
    fn from(value: SourceType) -> Self {
        match value {
            SourceType::Solidity => smart_contract_verifier::SourceType::Solidity,
            SourceType::Vyper => smart_contract_verifier::SourceType::Vyper,
            SourceType::Yul => smart_contract_verifier::SourceType::Yul,
        }
    }
}

/********** Match Type **********/

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchType {
    Unknown,
    Partial,
    Full,
}

impl From<smart_contract_verifier::MatchType> for MatchType {
    fn from(value: smart_contract_verifier::MatchType) -> Self {
        match value {
            smart_contract_verifier::MatchType::Unspecified => MatchType::Unknown,
            smart_contract_verifier::MatchType::Partial => MatchType::Partial,
            smart_contract_verifier::MatchType::Full => MatchType::Full,
        }
    }
}

/********** Source **********/

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub file_name: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub compiler_settings: String,
    pub source_type: SourceType,
    pub source_files: BTreeMap<String, String>,
    pub abi: Option<String>,
    pub constructor_arguments: Option<String>,
    pub match_type: MatchType,
    pub compilation_artifacts: Option<String>,
    pub creation_input_artifacts: Option<String>,
    pub deployed_bytecode_artifacts: Option<String>,

    pub raw_creation_input: Vec<u8>,
    pub raw_deployed_bytecode: Vec<u8>,
    pub creation_input_parts: Vec<BytecodePart>,
    pub deployed_bytecode_parts: Vec<BytecodePart>,
}

impl
    TryFrom<(
        smart_contract_verifier::Source,
        smart_contract_verifier::ExtraData,
    )> for Source
{
    type Error = anyhow::Error;

    fn try_from(
        (source, extra_data): (
            smart_contract_verifier::Source,
            smart_contract_verifier::ExtraData,
        ),
    ) -> Result<Self, Self::Error> {
        let parse_local_parts = |local_parts: Vec<smart_contract_verifier::BytecodePart>,
                                 bytecode_type: &str|
         -> Result<(Vec<BytecodePart>, Vec<u8>), anyhow::Error> {
            let parts = local_parts
                .into_iter()
                .map(|part| {
                    BytecodePart::try_from(part).map_err(|err| {
                        anyhow::anyhow!("error while decoding local {}: {}", bytecode_type, err,)
                            .context("verifier service connection")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            let raw_input = parts
                .iter()
                .flat_map(|part| part.data().to_vec())
                .collect::<Vec<_>>();

            Ok((parts, raw_input))
        };

        let (creation_input_parts, raw_creation_input) =
            parse_local_parts(extra_data.local_creation_input_parts, "creation input")?;
        let (deployed_bytecode_parts, raw_deployed_bytecode) = parse_local_parts(
            extra_data.local_deployed_bytecode_parts,
            "deployed bytecode",
        )?;

        let source_type = source.source_type().try_into()?;
        let match_type = source.match_type().into();
        Ok(Self {
            file_name: source.file_name,
            contract_name: source.contract_name,
            compiler_version: source.compiler_version,
            compiler_settings: source.compiler_settings,
            source_type,
            source_files: source.source_files,
            abi: source.abi,
            constructor_arguments: source.constructor_arguments,
            match_type,
            compilation_artifacts: source.compilation_artifacts,
            creation_input_artifacts: source.creation_input_artifacts,
            deployed_bytecode_artifacts: source.deployed_bytecode_artifacts,
            raw_creation_input,
            raw_deployed_bytecode,
            creation_input_parts,
            deployed_bytecode_parts,
        })
    }
}

/// The same as [`Source`] but processed to be inserted into the database.
/// The processing consists of converting all JSON stored types into [`serde_json::Value`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseReadySource {
    pub file_name: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub compiler_settings: serde_json::Value,
    pub source_type: SourceType,
    pub source_files: BTreeMap<String, String>,
    pub abi: Option<serde_json::Value>,
    pub compilation_artifacts: Option<serde_json::Value>,
    pub creation_input_artifacts: Option<serde_json::Value>,
    pub deployed_bytecode_artifacts: Option<serde_json::Value>,

    pub raw_creation_input: Vec<u8>,
    pub raw_deployed_bytecode: Vec<u8>,
    pub creation_input_parts: Vec<BytecodePart>,
    pub deployed_bytecode_parts: Vec<BytecodePart>,
}

impl TryFrom<Source> for DatabaseReadySource {
    type Error = anyhow::Error;

    fn try_from(value: Source) -> Result<Self, Self::Error> {
        let abi = value
            .abi
            .map(|abi| serde_json::from_str(&abi).context("deserialize abi into json value"))
            .transpose()?;
        let compiler_settings: serde_json::Value =
            serde_json::from_str(&value.compiler_settings)
                .context("deserialize compiler settings into json value")?;
        let compilation_artifacts: Option<serde_json::Value> = value
            .compilation_artifacts
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .context("deserialize compilation artifacts into json value")?;
        let creation_input_artifacts: Option<serde_json::Value> = value
            .creation_input_artifacts
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .context("deserialize creation input artifacts into json value")?;
        let deployed_bytecode_artifacts: Option<serde_json::Value> = value
            .deployed_bytecode_artifacts
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .context("deserialize deployed bytecode artifacts into json value")?;

        Ok(Self {
            file_name: value.file_name,
            contract_name: value.contract_name,
            compiler_version: value.compiler_version,
            compiler_settings,
            source_type: value.source_type,
            source_files: value.source_files,
            abi,
            compilation_artifacts,
            creation_input_artifacts,
            deployed_bytecode_artifacts,
            raw_creation_input: value.raw_creation_input,
            raw_deployed_bytecode: value.raw_deployed_bytecode,
            creation_input_parts: value.creation_input_parts,
            deployed_bytecode_parts: value.deployed_bytecode_parts,
        })
    }
}

/********** Verification Request **********/

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationMetadata {
    pub chain_id: Option<i64>,
    pub contract_address: Option<bytes::Bytes>,
    pub transaction_hash: Option<bytes::Bytes>,
    pub block_number: Option<i64>,
    pub transaction_index: Option<i64>,
    pub deployer: Option<bytes::Bytes>,
    pub creation_code: Option<bytes::Bytes>,
    pub runtime_code: Option<bytes::Bytes>,
}

impl From<VerificationMetadata> for smart_contract_verifier::VerificationMetadata {
    fn from(value: VerificationMetadata) -> Self {
        let chain_id = value.chain_id.map(|id| format!("{}", id));
        let contract_address = value
            .contract_address
            .map(|address| blockscout_display_bytes::Bytes::from(address).to_string());
        Self {
            chain_id,
            contract_address,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VerificationRequest<T> {
    pub bytecode: String,
    pub bytecode_type: BytecodeType,
    pub compiler_version: String,
    #[serde(flatten)]
    pub content: T,
    pub metadata: Option<VerificationMetadata>,
    #[serde(skip_serializing)]
    pub is_authorized: bool,
}

/********** Verification Type **********/

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum VerificationType {
    MultiPartFiles,
    StandardJson,
}

impl From<VerificationType> for sea_orm_active_enums::VerificationType {
    fn from(value: VerificationType) -> Self {
        match value {
            VerificationType::MultiPartFiles => {
                sea_orm_active_enums::VerificationType::MultiPartFiles
            }
            VerificationType::StandardJson => sea_orm_active_enums::VerificationType::StandardJson,
        }
    }
}
