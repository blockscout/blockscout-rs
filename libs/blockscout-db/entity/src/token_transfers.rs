//! `SeaORM` Entity, @generated by sea-orm-codegen 1.0.1

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "token_transfers")]
pub struct Model {
    #[sea_orm(
        primary_key,
        auto_increment = false,
        column_type = "VarBinary(StringLen::None)"
    )]
    pub transaction_hash: Vec<u8>,
    #[sea_orm(primary_key, auto_increment = false)]
    pub log_index: i32,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub from_address_hash: Vec<u8>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub to_address_hash: Vec<u8>,
    pub amount: Option<BigDecimal>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub token_contract_address_hash: Vec<u8>,
    pub inserted_at: DateTime,
    pub updated_at: DateTime,
    pub block_number: Option<i32>,
    #[sea_orm(
        primary_key,
        auto_increment = false,
        column_type = "VarBinary(StringLen::None)"
    )]
    pub block_hash: Vec<u8>,
    pub amounts: Option<Vec<Decimal>>,
    pub token_ids: Option<Vec<Decimal>>,
    pub token_type: Option<String>,
    pub block_consensus: Option<bool>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::blocks::Entity",
        from = "Column::BlockHash",
        to = "super::blocks::Column::Hash",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Blocks,
    #[sea_orm(
        belongs_to = "super::transactions::Entity",
        from = "Column::TransactionHash",
        to = "super::transactions::Column::Hash",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Transactions,
}

impl Related<super::blocks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Blocks.def()
    }
}

impl Related<super::transactions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Transactions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
