use anyhow::anyhow;
use scoutcloud_entity::{auth_tokens, users};
use sea_orm::{
    prelude::*,
    sea_query::{Alias, Expr},
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, QueryFilter,
};
use thiserror::Error;
use tonic::codegen::http::HeaderMap;

const AUTH_TOKEN_NAME: &str = "x-api-key";

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("no token provided, use x-api-key header to provide token")]
    NoToken,
    #[error("token not found")]
    TokenNotFound,
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Clone, Debug)]
pub struct UserToken {
    pub user: users::Model,
    pub token: auth_tokens::Model,
}

pub async fn authenticate(
    db: &DatabaseConnection,
    headers: &HeaderMap,
) -> Result<UserToken, AuthError> {
    let token_value = headers
        .get(AUTH_TOKEN_NAME)
        .or(headers.get(&AUTH_TOKEN_NAME.to_uppercase()))
        .ok_or_else(|| AuthError::NoToken)?
        .to_str()
        .map_err(|e| anyhow::anyhow!(e))?;

    let (token, user) = auth_tokens::Entity::find()
        .find_also_related(users::Entity)
        .filter(auth_tokens::Column::Deleted.eq(false))
        .filter(
            Expr::col(auth_tokens::Column::TokenValue)
                .cast_as(Alias::new("text"))
                .eq(token_value),
        )
        .one(db)
        .await
        .map_err(|e| anyhow!(e))?
        .ok_or_else(|| AuthError::TokenNotFound)?;
    let user = user.ok_or_else(|| anyhow::anyhow!("token doesn't have user_id"))?;

    Ok(UserToken { user, token })
}

pub async fn create_token(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<auth_tokens::Model, DbErr> {
    let token = auth_tokens::ActiveModel {
        user_id: Set(user_id),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(token)
}

pub async fn delete_token(db: &DatabaseConnection, token_id: i32) -> Result<(), DbErr> {
    auth_tokens::Entity::update_many()
        .filter(auth_tokens::Column::Id.eq(token_id))
        .col_expr(auth_tokens::Column::Deleted, Expr::value(true))
        .exec(db)
        .await?;
    Ok(())
}
