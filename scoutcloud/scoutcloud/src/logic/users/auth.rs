use anyhow::Context;
use scoutcloud_entity::{auth_tokens, users};
use sea_orm::{
    prelude::*, sea_query::Expr, ActiveModelTrait, ActiveValue::Set, ColumnTrait,
    DatabaseConnection, QueryFilter,
};
use std::collections::HashMap;
use thiserror::Error;

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

pub async fn authenticate(
    db: &DatabaseConnection,
    headers: &HashMap<String, String>,
) -> Result<(auth_tokens::Model, users::Model), AuthError> {
    let token_value = headers
        .get(AUTH_TOKEN_NAME)
        .or(headers.get(&AUTH_TOKEN_NAME.to_uppercase()))
        .ok_or_else(|| AuthError::NoToken)?;

    let (token, user) = auth_tokens::Entity::find()
        .find_also_related(users::Entity)
        .filter(auth_tokens::Column::Deleted.eq(false))
        .filter(auth_tokens::Column::TokenValue.eq(token_value))
        .one(db)
        .await
        .context("fetching token")?
        .ok_or_else(|| AuthError::TokenNotFound)?;
    let user = user.ok_or_else(|| anyhow::anyhow!("token doesn't have user_id"))?;

    Ok((token, user))
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
