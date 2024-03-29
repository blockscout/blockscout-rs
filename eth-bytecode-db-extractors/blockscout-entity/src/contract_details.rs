//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.2

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "contract_details")]
pub struct Model {
    pub created_at: DateTime,
    pub modified_at: DateTime,
    #[sea_orm(
        primary_key,
        auto_increment = false,
        column_type = "Binary(BlobSize::Blob(None))"
    )]
    pub contract_address: Vec<u8>,
    #[sea_orm(primary_key, auto_increment = false)]
    pub chain_id: Decimal,
    #[sea_orm(column_type = "JsonBinary")]
    pub sources: Json,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub settings: Option<Json>,
    pub verified_via_sourcify: bool,
    pub optimization_enabled: Option<bool>,
    pub optimization_runs: Option<i64>,
    pub evm_version: Option<String>,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub libraries: Option<Json>,
    #[sea_orm(column_type = "Binary(BlobSize::Blob(None))", nullable)]
    pub creation_code: Option<Vec<u8>>,
    #[sea_orm(column_type = "Binary(BlobSize::Blob(None))")]
    pub runtime_code: Vec<u8>,
    #[sea_orm(column_type = "Binary(BlobSize::Blob(None))", nullable)]
    pub transaction_hash: Option<Vec<u8>>,
    pub block_number: Decimal,
    pub transaction_index: Option<Decimal>,
    #[sea_orm(column_type = "Binary(BlobSize::Blob(None))", nullable)]
    pub deployer: Option<Vec<u8>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contract_addresses::Entity",
        from = "Column::ContractAddress",
        to = "super::contract_addresses::Column::ChainId",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    ContractAddresses,
}

impl Related<super::contract_addresses::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContractAddresses.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
