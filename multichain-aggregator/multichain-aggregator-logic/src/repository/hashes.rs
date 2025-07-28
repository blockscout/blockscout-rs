use super::paginate_cursor;
use crate::types::{ChainId, hashes::Hash};
use alloy_primitives::BlockHash;
use entity::{
    hashes::{Column, Entity, Model},
    sea_orm_active_enums as db_enum,
};
use sea_orm::{
    ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, IntoActiveModel,
    QueryFilter, QueryTrait, sea_query::OnConflict,
};

pub async fn upsert_many<C>(db: &C, hashes: Vec<Hash>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let hashes = hashes.into_iter().map(|hash| {
        let model: Model = hash.into();
        let mut active = model.into_active_model();
        active.created_at = NotSet;
        active
    });

    Entity::insert_many(hashes)
        .on_conflict(
            OnConflict::columns([Column::Hash, Column::ChainId])
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}

// Because (`hash`, `chain_id`) is a primary key
// we can paginate by `chain_id` only, as `hash` is always provided
pub async fn list<C>(
    db: &C,
    hash: BlockHash,
    hash_type: Option<db_enum::HashType>,
    chain_ids: Vec<ChainId>,
    page_size: u64,
    page_token: Option<ChainId>,
) -> Result<(Vec<Model>, Option<ChainId>), DbErr>
where
    C: ConnectionTrait,
{
    let c = Entity::find()
        .filter(Column::Hash.eq(hash.as_slice()))
        .apply_if(hash_type, |q, hash_type| {
            q.filter(Column::HashType.eq(hash_type))
        })
        .apply_if(
            (!chain_ids.is_empty()).then_some(chain_ids),
            |q, chain_ids| q.filter(Column::ChainId.is_in(chain_ids)),
        )
        .cursor_by(Column::ChainId);

    paginate_cursor(db, c, page_size, page_token, |u| u.chain_id).await
}
