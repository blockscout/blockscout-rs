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

#[cfg(test)]
mod test {
    use super::*;
    use crate::{logic::UserToken, tests_utils};
    use pretty_assertions::assert_eq;
    use rust_decimal::Decimal;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};

    #[tokio::test]
    async fn balance_change_works() {
        let db = tests_utils::init::test_db("test", "balance_change_works").await;
        let conn = db.client();
        tests_utils::mock::insert_default_data(conn.as_ref())
            .await
            .expect("insert_default_data failed");
        let user_token = UserToken::get(conn.as_ref(), 1)
            .await
            .expect("get user token failed");

        let profile = get_profile(conn.as_ref(), &user_token)
            .await
            .expect("get profile failed");
        // there is initial balance in default mock data
        assert_eq!(profile.balance, "100");

        scoutcloud_entity::balance_expenses::ActiveModel {
            user_id: Set(user_token.user.id),
            expense_amount: Set(Decimal::new(25, 0)),
            deployment_id: Set(1),
            hours: Set(1),
            ..Default::default()
        }
        .insert(conn.as_ref())
        .await
        .expect("insert balance expense failed");
        scoutcloud_entity::balance_changes::ActiveModel {
            user_id: Set(user_token.user.id),
            amount: Set(Decimal::new(-10, 0)),
            ..Default::default()
        }
        .insert(conn.as_ref())
        .await
        .expect("insert balance change failed");

        let user_token = UserToken::get(conn.as_ref(), 1)
            .await
            .expect("get user token failed");
        let profile = get_profile(conn.as_ref(), &user_token)
            .await
            .expect("get profile failed");
        assert_eq!(profile.balance, "65");
    }
}
