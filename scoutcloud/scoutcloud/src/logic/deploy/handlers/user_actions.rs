use crate::logic::UserToken;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait, NotSet};

pub(crate) async fn user_action<C>(
    db: &C,
    user_token: &UserToken,
    action: &str,
    maybe_data: Option<serde_json::Value>,
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    scoutcloud_entity::user_actions::ActiveModel {
        token_id: Set(user_token.token.id),
        action: Set(action.to_string()),
        data: maybe_data.map(Set).unwrap_or(NotSet),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}
