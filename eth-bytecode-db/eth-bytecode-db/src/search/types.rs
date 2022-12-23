use bytes::Bytes;
use entity::sea_orm_active_enums::BytecodeType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeRemote {
    pub bytecode_type: BytecodeType,
    pub data: Bytes,
}
