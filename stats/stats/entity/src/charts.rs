//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use super::sea_orm_active_enums::{ChartResolution, ChartType};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "charts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub chart_type: ChartType,
    pub created_at: DateTimeWithTimeZone,
    pub last_updated_at: Option<DateTimeWithTimeZone>,
    pub resolution: ChartResolution,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::chart_data::Entity")]
    ChartData,
}

impl Related<super::chart_data::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChartData.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
