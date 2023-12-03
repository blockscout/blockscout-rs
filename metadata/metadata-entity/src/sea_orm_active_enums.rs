//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.6

use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(
    rs_type = "String",
    db_type = "Enum",
    enum_name = "note_severity_level"
)]
pub enum NoteSeverityLevel {
    #[sea_orm(string_value = "info")]
    Info,
    #[sea_orm(string_value = "warn")]
    Warn,
}
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "public_tag_type")]
pub enum PublicTagType {
    #[sea_orm(string_value = "generic")]
    Generic,
    #[sea_orm(string_value = "information")]
    Information,
    #[sea_orm(string_value = "name")]
    Name,
}
