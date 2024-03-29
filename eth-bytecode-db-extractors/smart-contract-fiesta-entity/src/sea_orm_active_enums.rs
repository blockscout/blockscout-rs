//! `SeaORM` Entity. Generated by sea-orm-codegen 0.11.2

use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "status")]
pub enum Status {
    #[sea_orm(string_value = "error")]
    Error,
    #[sea_orm(string_value = "in_process")]
    InProcess,
    #[sea_orm(string_value = "success")]
    Success,
    #[sea_orm(string_value = "waiting")]
    Waiting,
}
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(
    rs_type = "String",
    db_type = "Enum",
    enum_name = "verification_method"
)]
pub enum VerificationMethod {
    #[sea_orm(string_value = "solidity_multiple")]
    SolidityMultiple,
    #[sea_orm(string_value = "solidity_single")]
    SoliditySingle,
    #[sea_orm(string_value = "solidity_standard")]
    SolidityStandard,
    #[sea_orm(string_value = "vyper_single")]
    VyperSingle,
}
