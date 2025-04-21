use crate::types::{api_keys::ApiKey, ChainId};
use entity::api_keys::{Column, Entity};
use sea_orm::{prelude::Uuid, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};

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
