use crate::{logic::Instance, uuid_eq};
use scoutcloud_entity::{auth_tokens, server_specs, users};
use sea_orm::{
    prelude::*, sea_query::Expr, ActiveModelTrait, ActiveValue::Set, ColumnTrait, QueryFilter,
};
use std::ops::Sub;
use thiserror::Error;
use tonic::codegen::http::HeaderMap;

const AUTH_TOKEN_NAME: &str = "x-api-key";

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("no token provided, use x-api-key header to provide token")]
    NoToken,
    #[error("token not found")]
    TokenNotFound,
    #[error("requested resource not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("db error: {0}")]
    Db(#[from] DbErr),
}

#[derive(Clone, Debug)]
pub struct UserToken {
    pub user: users::Model,
    pub token: auth_tokens::Model,
}

impl UserToken {
    pub async fn try_from_http_headers<C>(db: &C, headers: &HeaderMap) -> Result<Self, AuthError>
    where
        C: ConnectionTrait,
    {
        let token_value = headers
            .get(AUTH_TOKEN_NAME)
            .or(headers.get(&AUTH_TOKEN_NAME.to_uppercase()))
            .ok_or_else(|| AuthError::NoToken)?
            .to_str()
            .map_err(|e| anyhow::anyhow!(e))?;

        Self::try_from_token_value(db, token_value).await
    }

    pub async fn try_from_token_value<C>(
        db: &C,
        token_value: impl Into<String>,
    ) -> Result<Self, AuthError>
    where
        C: ConnectionTrait,
    {
        let (token, user) = auth_tokens::Entity::find()
            .find_also_related(users::Entity)
            .filter(auth_tokens::Column::Deleted.eq(false))
            .filter(uuid_eq!(
                auth_tokens::Column::TokenValue,
                token_value.into()
            ))
            .one(db)
            .await?
            .ok_or_else(|| AuthError::TokenNotFound)?;
        let user = user.ok_or_else(|| anyhow::anyhow!("token doesn't have user_id"))?;
        Ok(Self { user, token })
    }

    pub async fn has_access_to_instance(&self, instance: &Instance) -> Result<(), AuthError> {
        if self.user.is_superuser || instance.model.creator_token_id == self.token.id {
            Ok(())
        } else {
            Err(AuthError::Unauthorized)
        }
    }

    pub async fn allowed_to_deploy_for_hours(
        &self,
        hours: u64,
        server_spec: &server_specs::Model,
    ) -> Result<(), AuthError> {
        if self.user.is_superuser {
            return Ok(());
        }
        let hours = Decimal::new(hours as i64, 0);
        if self
            .user
            .balance
            .sub(hours * server_spec.cost_per_hour)
            .is_sign_negative()
        {
            return Err(AuthError::InsufficientBalance);
        }
        Ok(())
    }

    pub async fn create<C>(db: &C, user_id: i32) -> Result<Self, AuthError>
    where
        C: ConnectionTrait,
    {
        let token = auth_tokens::ActiveModel {
            user_id: Set(user_id),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Self::try_from_token_value(db, token.token_value).await
    }

    pub async fn delete<C>(self, db: &C) -> Result<(), AuthError>
    where
        C: ConnectionTrait,
    {
        auth_tokens::Entity::update_many()
            .filter(uuid_eq!(auth_tokens::Column::Id, self.token.id))
            .col_expr(auth_tokens::Column::Deleted, Expr::value(true))
            .exec(db)
            .await?;
        Ok(())
    }
}
