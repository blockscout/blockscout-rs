//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.8

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "massive_blocks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub number: i64,
    pub inserted_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
