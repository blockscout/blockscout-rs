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

/********** Match Type **********/

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchType {
    Partial,
    Full,
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
pub struct VerificationRequest<T> {
    pub bytecode: String,
    pub bytecode_type: BytecodeType,
    pub compiler_version: String,
    #[serde(flatten)]
    pub content: T,
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
