use crate::{
    logic::{AuthError, UserToken},
    server::proto,
};
use sea_orm::{prelude::*, ConnectionTrait, DatabaseConnection, DbErr, QueryOrder, QuerySelect};

const RECENT_ACTIONS_LIMIT: u64 = 50;

pub async fn get_profile(
    db: &DatabaseConnection,
    user_token: &UserToken,
) -> Result<proto::UserProfileInternal, AuthError> {
    let recent_actions = get_user_actions(db, user_token, RECENT_ACTIONS_LIMIT).await?;
    let profile = proto::UserProfileInternal {
        email: user_token.user.email.clone(),
        project_title: user_token.user.project_title.clone(),
        balance: user_token.user.balance.to_string(),
        created_at: user_token.user.created_at.to_string(),
        recent_actions,
    };
    Ok(profile)
}

async fn get_user_actions<C>(
    db: &C,
    user_token: &UserToken,
    limit: u64,
) -> Result<Vec<proto::UserAction>, DbErr>
where
    C: ConnectionTrait,
{
    let actions = scoutcloud_entity::user_actions::Entity::find()
        .filter(scoutcloud_entity::user_actions::Column::TokenId.eq(user_token.token.id))
        .find_also_related(scoutcloud_entity::instances::Entity)
        .order_by_desc(scoutcloud_entity::user_actions::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await?
        .into_iter()
        .map(|(action, maybe_instance)| proto::UserAction {
            action: action.action,
            instance_id: maybe_instance.map(|i| i.external_id.to_string()),
            timestamp: action.created_at.to_string(),
        })
        .collect();

    Ok(actions)
}
