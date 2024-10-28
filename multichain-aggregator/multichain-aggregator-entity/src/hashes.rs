//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.0

use super::sea_orm_active_enums::HashType;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "hashes")]
pub struct Model {
    #[sea_orm(
        primary_key,
        auto_increment = false,
        column_type = "VarBinary(StringLen::None)"
    )]
    pub hash: Vec<u8>,
    #[sea_orm(primary_key, auto_increment = false)]
    pub chain_id: i32,
    pub hash_type: HashType,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::chains::Entity",
        from = "Column::ChainId",
        to = "super::chains::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Chains,
}

impl Related<super::chains::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Chains.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
