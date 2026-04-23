use bytes::Bytes;
use entity::sea_orm_active_enums;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeRemote {
    pub bytecode_type: BytecodeType,
    pub data: Bytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BytecodeType {
    CreationCode,
    RuntimeCode,
    CreationCodeWithoutConstructor,
}

impl From<sea_orm_active_enums::BytecodeType> for BytecodeType {
    fn from(value: sea_orm_active_enums::BytecodeType) -> Self {
        match value {
            sea_orm_active_enums::BytecodeType::CreationInput => BytecodeType::CreationCode,
            sea_orm_active_enums::BytecodeType::DeployedBytecode => BytecodeType::RuntimeCode,
        }
    }
}

impl From<BytecodeType> for sea_orm_active_enums::BytecodeType {
    fn from(value: BytecodeType) -> Self {
        match value {
            BytecodeType::CreationCode | BytecodeType::CreationCodeWithoutConstructor => {
                sea_orm_active_enums::BytecodeType::CreationInput
            }
            BytecodeType::RuntimeCode => sea_orm_active_enums::BytecodeType::DeployedBytecode,
        }
    }
}

impl Display for BytecodeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BytecodeType::CreationCode => f.write_str("creation_code"),
            BytecodeType::RuntimeCode => f.write_str("runtime_code"),
            BytecodeType::CreationCodeWithoutConstructor => {
                f.write_str("creation_code_without_constructor")
            }
        }
    }
}
