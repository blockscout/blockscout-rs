//! `SeaORM` Entity, @generated by sea-orm-codegen 1.0.1

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "account_public_tags_requests")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub identity_id: Option<i64>,
    pub company: Option<String>,
    pub website: Option<String>,
    pub tags: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub additional_comment: Option<String>,
    pub request_type: Option<String>,
    pub is_owner: Option<bool>,
    #[sea_orm(column_type = "Text", nullable)]
    pub remove_reason: Option<String>,
    pub request_id: Option<String>,
    pub inserted_at: DateTime,
    pub updated_at: DateTime,
    pub addresses: Option<Vec<Vec<u8>>>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)", nullable)]
    pub email: Option<Vec<u8>>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)", nullable)]
    pub full_name: Option<Vec<u8>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::account_identities::Entity",
        from = "Column::IdentityId",
        to = "super::account_identities::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    AccountIdentities,
}

impl Related<super::account_identities::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccountIdentities.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
