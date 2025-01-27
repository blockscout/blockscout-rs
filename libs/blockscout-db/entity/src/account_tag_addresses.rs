//! `SeaORM` Entity, @generated by sea-orm-codegen 1.0.1

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "account_tag_addresses")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub identity_id: Option<i64>,
    pub inserted_at: DateTime,
    pub updated_at: DateTime,
    #[sea_orm(column_type = "VarBinary(StringLen::None)", nullable)]
    pub address_hash_hash: Option<Vec<u8>>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)", nullable)]
    pub name: Option<Vec<u8>>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)", nullable)]
    pub address_hash: Option<Vec<u8>>,
    pub user_created: Option<bool>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::account_identities::Entity",
        from = "Column::IdentityId",
        to = "super::account_identities::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    AccountIdentities,
}

impl Related<super::account_identities::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccountIdentities.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
