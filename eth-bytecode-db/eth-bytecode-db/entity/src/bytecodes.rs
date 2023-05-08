//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.2

use super::sea_orm_active_enums::BytecodeType;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "bytecodes")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub source_id: i64,
    pub bytecode_type: BytecodeType,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sources::Entity",
        from = "Column::SourceId",
        to = "super::sources::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Sources,
    #[sea_orm(has_many = "super::bytecode_parts::Entity")]
    BytecodeParts,
}

impl Related<super::sources::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Sources.def()
    }
}

impl Related<super::bytecode_parts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BytecodeParts.def()
    }
}

impl Related<super::parts::Entity> for Entity {
    fn to() -> RelationDef {
        super::bytecode_parts::Relation::Parts.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::bytecode_parts::Relation::Bytecodes.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
