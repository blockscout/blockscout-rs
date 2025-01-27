use crate::{error::ServiceError, types::hashes::Hash};
use alloy_primitives::BlockHash;
use entity::{
    hashes::{ActiveModel, Column, Entity, Model},
    sea_orm_active_enums as db_enum,
};
use sea_orm::{
    sea_query::OnConflict, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DbErr, EntityTrait,
    QueryFilter,
};

pub async fn upsert_many<C>(db: &C, hashes: Vec<Hash>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    if hashes.is_empty() {
        return Ok(());
    }

    let hashes = hashes.into_iter().map(|hash| {
        let model: Model = hash.into();
        let mut active: ActiveModel = model.into();
        active.created_at = NotSet;
        active
    });

    let res = Entity::insert_many(hashes)
        .on_conflict(
            OnConflict::columns([Column::Hash, Column::ChainId])
                .do_nothing()
                .to_owned(),
        )
        .exec(db)
        .await;

    match res {
        Ok(_) | Err(DbErr::RecordNotInserted) => Ok(()),
        Err(err) => Err(err),
    }
}

pub async fn find_by_hash<C>(
    db: &C,
    hash: BlockHash,
) -> Result<(Vec<Hash>, Vec<Hash>), ServiceError>
where
    C: ConnectionTrait,
{
    let res = Entity::find()
        .filter(Column::Hash.eq(hash.as_slice()))
        .all(db)
        .await?
        .into_iter()
        .map(Hash::try_from)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .partition(|h| h.hash_type == db_enum::HashType::Block);

    Ok(res)
}
