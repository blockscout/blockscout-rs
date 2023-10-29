//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.2

use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "_job_status")]
pub enum JobStatus {
    #[sea_orm(string_value = "error")]
    Error,
    #[sea_orm(string_value = "in_process")]
    InProcess,
    #[sea_orm(string_value = "success")]
    Success,
    #[sea_orm(string_value = "waiting")]
    Waiting,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "_job_queue")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub created_at: DateTime,
    pub modified_at: DateTime,
    pub status: JobStatus,
    pub log: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
