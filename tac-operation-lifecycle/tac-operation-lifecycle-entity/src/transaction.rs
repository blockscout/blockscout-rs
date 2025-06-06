//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.5

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "transaction")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub stage_id: i32,
    pub hash: String,
    pub inserted_at: DateTime,
    pub blockchain_type: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::operation_stage::Entity",
        from = "Column::StageId",
        to = "super::operation_stage::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    OperationStage,
}

impl Related<super::operation_stage::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OperationStage.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
