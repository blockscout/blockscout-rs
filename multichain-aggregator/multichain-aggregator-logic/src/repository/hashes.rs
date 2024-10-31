use crate::types::hashes::Hash;
use entity::hashes::{ActiveModel, Column, Entity, Model};
use sea_orm::{sea_query::OnConflict, ActiveValue::NotSet, ConnectionTrait, DbErr, EntityTrait};

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
