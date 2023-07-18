use super::smart_contract_verifier;
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

    pub raw_creation_input: Vec<u8>,
    pub raw_deployed_bytecode: Vec<u8>,
    pub creation_input_parts: Vec<BytecodePart>,
    pub deployed_bytecode_parts: Vec<BytecodePart>,
}

/********** Verification Request **********/

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationMetadata {
    pub chain_id: Option<i64>,
    pub contract_address: Option<bytes::Bytes>,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRequest<T> {
    pub bytecode: String,
    pub bytecode_type: BytecodeType,
    pub compiler_version: String,
    #[serde(flatten)]
    pub content: T,
    pub metadata: Option<VerificationMetadata>,
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
