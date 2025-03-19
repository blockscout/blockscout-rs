//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.0

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "contracts")]
pub struct Model {
    #[sea_orm(
        primary_key,
        auto_increment = false,
        column_type = "VarBinary(StringLen::None)"
    )]
    pub address: Vec<u8>,
    pub inserted_at: DateTime,
    pub updated_at: DateTime,
    pub is_verified: Option<bool>,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub data: Option<Json>,
    #[sea_orm(column_name = "_job_id")]
    pub job_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::job_queue::Entity",
        from = "Column::JobId",
        to = "super::job_queue::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    JobQueue,
}

impl Related<super::job_queue::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::JobQueue.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
