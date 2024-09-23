//! `SeaORM` Entity, @generated by sea-orm-codegen 1.0.1

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "address_names")]
pub struct Model {
    #[sea_orm(column_type = "Binary(BlobSize::Blob(None))", unique)]
    pub address_hash: Vec<u8>,
    pub name: String,
    pub primary: bool,
    pub inserted_at: DateTime,
    pub updated_at: DateTime,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub metadata: Option<Json>,
    #[sea_orm(primary_key)]
    pub id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
