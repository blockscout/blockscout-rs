use crate::{
    logic::{users::register_promo::try_find_promo, AuthError, UserToken},
    server::proto,
};
use sea_orm::{
    prelude::*, ActiveValue::Set, ConnectionTrait, DatabaseConnection, DbErr, NotSet, QueryOrder,
    QuerySelect, TransactionTrait,
};
const RECENT_ACTIONS_LIMIT: u64 = 50;

pub async fn register_profile(
    db: &DatabaseConnection,
    profile: &proto::RegisterProfileRequestInternal,
) -> Result<proto::RegisterProfileResponseInternal, AuthError> {
    let tx = db.begin().await?;
    let maybe_promo = if let Some(promo) = &profile.promo {
        Some(try_find_promo(&tx, promo).await?)
    } else {
        None
    };
    let user = scoutcloud_entity::users::ActiveModel {
        project_title: Set(Some(profile.project_title.to_string())),
        email: Set(profile.email.to_string()),
        max_instances: maybe_promo
            .as_ref()
            .map(|p| Set(p.user_max_instances))
            .unwrap_or(NotSet),
        ..Default::default()
    }
    .insert(&tx)
    .await?;
    if let Some(promo) = maybe_promo {
        scoutcloud_entity::balance_changes::ActiveModel {
            user_id: Set(user.id),
            amount: Set(promo.user_initial_balance),
            note: Set(Some(format!("From promo code: {}", promo.code))),
            register_promo_id: Set(Some(promo.id)),
            ..Default::default()
        }
        .insert(&tx)
        .await?;
    }
    let token = create_auth_token(&tx, user.id, "initial token").await?;
    let user_token = UserToken::try_from_token_value(&tx, &token.token_value).await?;
    let profile = get_profile(&tx, &user_token).await?;
    tx.commit().await?;

    Ok(proto::RegisterProfileResponseInternal {
        profile: Some(profile),
        initial_token: Some(token),
    })
}

pub async fn get_profile<C: ConnectionTrait>(
    db: &C,
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

pub async fn create_auth_token<C: ConnectionTrait>(
    db: &C,
    user_id: i32,
    token_name: &str,
) -> Result<proto::AuthTokenInternal, AuthError> {
    let user_token = UserToken::create(db, user_id, token_name).await?;
    Ok(proto::AuthTokenInternal {
        name: user_token.token.name,
        token_value: user_token.token.token_value.to_string(),
        created_at: user_token.token.created_at.to_string(),
    })
}

pub async fn get_auth_tokens<C: ConnectionTrait>(
    db: &C,
    user_token: &UserToken,
) -> Result<Vec<proto::AuthTokenInternal>, AuthError> {
    let tokens = scoutcloud_entity::auth_tokens::Entity::find()
        .filter(scoutcloud_entity::auth_tokens::Column::UserId.eq(user_token.user.id))
        .filter(scoutcloud_entity::auth_tokens::Column::Deleted.eq(false))
        .all(db)
        .await?;
    Ok(tokens
        .into_iter()
        .map(|t| proto::AuthTokenInternal {
            name: t.name,
            token_value: t.token_value.to_string(),
            created_at: t.created_at.to_string(),
        })
        .collect())
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
    use std::str::FromStr;

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

    #[tokio::test]
    async fn register_profile_works() {
        let db = tests_utils::init::test_db("test", "register_profile_works").await;
        let conn = db.client();
        let email = "mail@mail.com".to_string();
        let project_title = "test".to_string();
        let profile = proto::RegisterProfileRequestInternal {
            email: email.clone(),
            project_title: project_title.clone(),
            promo: None,
        };
        // test without promo code
        let response = register_profile(conn.as_ref(), &profile)
            .await
            .expect("register profile failed");
        let token = response.initial_token.unwrap();
        let profile = response.profile.unwrap();

        Uuid::from_str(&token.token_value).expect("invalid uuid");
        assert_eq!(token.name, "initial token");
        assert_eq!(profile.project_title, Some(project_title.clone()));
        assert_eq!(profile.email, email);
        assert_eq!(profile.balance, "0");

        scoutcloud_entity::register_promo::ActiveModel {
            name: Set("promo1".to_string()),
            code: Set("XX42XX".to_string()),
            user_max_instances: Set(1),
            user_initial_balance: Set(Decimal::new(100, 0)),
            max_activations: Set(1),
            ..Default::default()
        }
        .insert(conn.as_ref())
        .await
        .expect("insert promo failed");

        // test promo code
        let email = "mail2@mail.com".to_string();
        let profile = proto::RegisterProfileRequestInternal {
            email: email.clone(),
            project_title: project_title.clone(),
            promo: Some("XX42XX".to_string()),
        };
        let response = register_profile(conn.as_ref(), &profile)
            .await
            .expect("register profile failed");

        let profile = response.profile.unwrap();
        let token = response.initial_token.unwrap();
        assert_eq!(profile.balance, "100");

        let actual_max_instances =
            UserToken::try_from_token_value(conn.as_ref(), &token.token_value)
                .await
                .expect("get user token failed")
                .user
                .max_instances;
        assert_eq!(actual_max_instances, 1);

        // test promo code already used
        let profile = proto::RegisterProfileRequestInternal {
            email: email.clone(),
            project_title: project_title.clone(),
            promo: Some("XX42XX".to_string()),
        };
        register_profile(conn.as_ref(), &profile)
            .await
            .expect_err("register profile should fail");
    }
}
