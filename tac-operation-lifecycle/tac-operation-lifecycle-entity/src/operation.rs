//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.0

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "operation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub operation_type: Option<String>,
    pub timestamp: i64,
    pub next_retry: Option<i64>,
    pub status: i32,
    pub retry_count: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::operation_stage::Entity")]
    OperationStage,
}

impl Related<super::operation_stage::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OperationStage.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
