use crate::logic::{Deployment, Instance, UserToken};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait, NotSet};
use serde::Serialize;
use serde_json::json;
use serde_plain::derive_display_from_serialize;
use std::fmt::Display;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserActionType {
    CreateInstance,
    DeleteInstance,
    UpdateInstanceConfig,
    UpdateInstanceConfigPartial,
    StartInstance,
    StopInstance,
    RestartInstance,
}
derive_display_from_serialize!(UserActionType);

pub(crate) async fn log_user_action<C>(
    db: &C,
    user_token: &UserToken,
    action: impl Display,
    instance_id: Option<i32>,
    maybe_data: Option<serde_json::Value>,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let action = action.to_string();
    tracing::info!(
        action = action,
        user_id = user_token.user.id,
        user_email = user_token.user.email,
        "user action '{action}'"
    );
    scoutcloud_entity::user_actions::ActiveModel {
        token_id: Set(user_token.token.id),
        action: Set(action.to_string()),
        instance_id: Set(instance_id),
        data: maybe_data.map(Set).unwrap_or(NotSet),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

pub(crate) async fn log_create_instance(
    db: &impl ConnectionTrait,
    user_token: &UserToken,
    instance: &Instance,
    config: &serde_json::Value,
) -> Result<(), sea_orm::DbErr> {
    log_user_action(
        db,
        user_token,
        UserActionType::CreateInstance,
        Some(instance.model.id),
        Some(json!({
            "instance_slug": instance.model.slug,
            "instance_uuid": instance.model.external_id,
            "config": config,
        })),
    )
    .await?;
    Ok(())
}

pub(crate) async fn log_update_config(
    db: &impl ConnectionTrait,
    user_token: &UserToken,
    instance: &Instance,
    old_config: &serde_json::Value,
    new_config: &serde_json::Value,
    is_partial: bool,
) -> Result<(), sea_orm::DbErr> {
    let action = if is_partial {
        UserActionType::UpdateInstanceConfigPartial
    } else {
        UserActionType::UpdateInstanceConfig
    };
    log_user_action(
        db,
        user_token,
        action,
        Some(instance.model.id),
        Some(json!({
            "instance_slug": instance.model.slug,
            "instance_uuid": instance.model.external_id,
            "old_config": old_config,
            "new_config": new_config,
        })),
    )
    .await?;
    Ok(())
}

pub(crate) async fn log_start_instance(
    db: &impl ConnectionTrait,
    user_token: &UserToken,
    instance: &Instance,
    deployment: &Deployment,
) -> Result<(), sea_orm::DbErr> {
    log_instance_action(
        db,
        user_token,
        instance,
        deployment,
        UserActionType::StartInstance,
    )
    .await
}

pub(crate) async fn log_stop_instance(
    db: &impl ConnectionTrait,
    user_token: &UserToken,
    instance: &Instance,
    deployment: &Deployment,
) -> Result<(), sea_orm::DbErr> {
    log_instance_action(
        db,
        user_token,
        instance,
        deployment,
        UserActionType::StopInstance,
    )
    .await
}

pub(crate) async fn log_restart_instance(
    db: &impl ConnectionTrait,
    user_token: &UserToken,
    instance: &Instance,
    deployment: &Deployment,
) -> Result<(), sea_orm::DbErr> {
    log_instance_action(
        db,
        user_token,
        instance,
        deployment,
        UserActionType::RestartInstance,
    )
    .await
}

pub(crate) async fn log_delete_instance(
    db: &impl ConnectionTrait,
    user_token: &UserToken,
    instance: &Instance,
) -> Result<(), sea_orm::DbErr> {
    log_user_action(
        db,
        user_token,
        UserActionType::DeleteInstance,
        Some(instance.model.id),
        Some(json!({
            "instance_slug": instance.model.slug,
            "instance_uuid": instance.model.external_id,
        })),
    )
    .await?;
    Ok(())
}

async fn log_instance_action(
    db: &impl ConnectionTrait,
    user_token: &UserToken,
    instance: &Instance,
    deployment: &Deployment,
    user_action_type: UserActionType,
) -> Result<(), sea_orm::DbErr> {
    log_user_action(
        db,
        user_token,
        user_action_type,
        Some(instance.model.id),
        Some(json!({
            "instance_slug": instance.model.slug,
            "instance_uuid": instance.model.external_id,
            "deployment_uuid": deployment.model.external_id,
        })),
    )
    .await?;
    Ok(())
}
