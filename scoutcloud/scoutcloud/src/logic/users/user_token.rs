use crate::{
    logic::{AuthError, Instance},
    uuid_eq,
};
use scoutcloud_entity::{auth_tokens, server_specs, users};
use sea_orm::{
    prelude::*, sea_query::Expr, ActiveModelTrait, ActiveValue::Set, ColumnTrait, QueryFilter,
};
use std::ops::Sub;
use tonic::codegen::http::HeaderMap;

const AUTH_TOKEN_NAME: &str = "x-api-key";

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

    pub async fn get<C>(db: &C, token_id: i32) -> Result<Self, AuthError>
    where
        C: ConnectionTrait,
    {
        let (token, maybe_user) = auth_tokens::Entity::find()
            .find_also_related(users::Entity)
            .filter(auth_tokens::Column::Id.eq(token_id))
            .one(db)
            .await?
            .ok_or_else(|| AuthError::TokenNotFound)?;
        let user = maybe_user.ok_or_else(|| anyhow::anyhow!("token doesn't have user_id"))?;

        Ok(Self { user, token })
    }

    pub fn has_access_to_instance(&self, instance: &Instance) -> Result<(), AuthError> {
        if self.user.is_superuser || instance.model.creator_id == self.user.id {
            Ok(())
        } else {
            Err(AuthError::Unauthorized("no to the instance".to_string()))
        }
    }

    pub async fn allowed_to_create_instance<C>(&self, db: &C) -> Result<(), AuthError>
    where
        C: ConnectionTrait,
    {
        if self.user.is_superuser {
            return Ok(());
        }
        if self.user.balance.is_sign_negative() {
            return Err(AuthError::InsufficientBalance);
        }
        let created_instances = Instance::count(db, self).await? as i32;
        if created_instances >= self.user.max_instances {
            return Err(AuthError::Unauthorized(format!(
                "max instances per user reached: {}",
                self.user.max_instances
            )));
        }

        Ok(())
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

    pub async fn create<C>(db: &C, user_id: i32, name: &str) -> Result<Self, AuthError>
    where
        C: ConnectionTrait,
    {
        let token = auth_tokens::ActiveModel {
            user_id: Set(user_id),
            name: Set(name.to_string()),
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
