//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.8

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "smart_contract_audit_reports")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub address_hash: Vec<u8>,
    pub is_approved: Option<bool>,
    pub submitter_name: String,
    pub submitter_email: String,
    pub is_project_owner: Option<bool>,
    pub project_name: String,
    pub project_url: String,
    pub audit_company_name: String,
    pub audit_report_url: String,
    pub audit_publish_date: Date,
    pub request_id: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub comment: Option<String>,
    pub inserted_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::smart_contracts::Entity",
        from = "Column::AddressHash",
        to = "super::smart_contracts::Column::AddressHash",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    SmartContracts,
}

impl Related<super::smart_contracts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SmartContracts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
