//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.2

use super::sea_orm_active_enums::{Language, Status};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "contract_addresses")]
pub struct Model {
    #[sea_orm(
        primary_key,
        auto_increment = false,
        column_type = "Binary(BlobSize::Blob(None))"
    )]
    pub contract_address: Vec<u8>,
    #[sea_orm(primary_key, auto_increment = false)]
    pub chain_id: Decimal,
    pub created_at: DateTime,
    pub modified_at: DateTime,
    pub verified_at: DateTimeWithTimeZone,
    pub language: Language,
    pub compiler_version: String,
    pub status: Status,
    pub log: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::contract_details::Entity")]
    ContractDetails,
}

impl Related<super::contract_details::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContractDetails.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
