//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.0

use super::sea_orm_active_enums::SourceType;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sources")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub source_type: SourceType,
    pub compiler_version: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub compiler_settings: Json,
    pub file_name: String,
    pub contract_name: String,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub abi: Option<Json>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub raw_creation_input: Vec<u8>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub raw_deployed_bytecode: Vec<u8>,
    pub file_ids_hash: Uuid,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub compilation_artifacts: Option<Json>,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub creation_input_artifacts: Option<Json>,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub deployed_bytecode_artifacts: Option<Json>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::bytecodes::Entity")]
    Bytecodes,
    #[sea_orm(has_many = "super::source_files::Entity")]
    SourceFiles,
    #[sea_orm(has_many = "super::verified_contracts::Entity")]
    VerifiedContracts,
}

impl Related<super::bytecodes::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bytecodes.def()
    }
}

impl Related<super::source_files::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SourceFiles.def()
    }
}

impl Related<super::verified_contracts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::VerifiedContracts.def()
    }
}

impl Related<super::files::Entity> for Entity {
    fn to() -> RelationDef {
        super::source_files::Relation::Files.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::source_files::Relation::Sources.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
