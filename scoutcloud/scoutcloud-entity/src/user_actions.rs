//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.12

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "user_actions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub token_id: i32,
    pub created_at: DateTime,
    pub action: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub data: Json,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::auth_tokens::Entity",
        from = "Column::TokenId",
        to = "super::auth_tokens::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    AuthTokens,
}

impl Related<super::auth_tokens::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AuthTokens.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
