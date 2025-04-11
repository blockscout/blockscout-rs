//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.8

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "smart_contracts_additional_sources")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub file_name: String,
    #[sea_orm(column_type = "Text")]
    pub contract_source_code: String,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub address_hash: Vec<u8>,
    pub inserted_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::smart_contracts::Entity",
        from = "Column::AddressHash",
        to = "super::smart_contracts::Column::AddressHash",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    SmartContracts,
}

impl Related<super::smart_contracts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SmartContracts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
