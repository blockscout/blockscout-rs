//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.13

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "dapps")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub chain_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub name: String,
    pub description: String,
    pub link: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
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
