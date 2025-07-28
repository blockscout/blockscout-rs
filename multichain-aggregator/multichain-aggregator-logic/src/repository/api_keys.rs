use crate::types::{ChainId, api_keys::ApiKey};
use entity::api_keys::{Column, Entity};
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, prelude::Uuid};

pub async fn find_by_key_and_chain_id(
    db: &DatabaseConnection,
    key: Uuid,
    chain_id: ChainId,
) -> Result<Option<ApiKey>, DbErr> {
    let api_key = Entity::find()
        .filter(Column::Key.eq(key))
        .filter(Column::ChainId.eq(chain_id))
        .one(db)
        .await?
        .map(ApiKey::from);

    Ok(api_key)
}
