use crate::types::{api_keys::ApiKey, ChainId};
use entity::api_keys::{ActiveModel, Column, Entity};
use sea_orm::{
    prelude::Uuid, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter,
};

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

pub async fn upsert_many<C>(db: &C, api_keys: Vec<ApiKey>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let api_keys = api_keys.into_iter().map(ActiveModel::from);

    Entity::insert_many(api_keys)
        .exec_without_returning(db)
        .await?;

    Ok(())
}
