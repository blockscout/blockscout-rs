use super::paginate_cursor;
use crate::types::{hashes::Hash, ChainId};
use alloy_primitives::BlockHash;
use entity::{
    hashes::{ActiveModel, Column, Entity, Model},
    sea_orm_active_enums as db_enum,
};
use sea_orm::{
    sea_query::OnConflict, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DbErr, EntityTrait,
    QueryFilter, QueryTrait,
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

// Because (`hash`, `chain_id`) is a primary key
// we can paginate by `chain_id` only, as `hash` is always provided
pub async fn list_hashes_paginated<C>(
    db: &C,
    hash: BlockHash,
    hash_type: Option<db_enum::HashType>,
    chain_id: Option<ChainId>,
    page_size: u64,
    page_token: Option<ChainId>,
) -> Result<(Vec<Model>, Option<ChainId>), DbErr>
where
    C: ConnectionTrait,
{
    let mut c = Entity::find()
        .filter(Column::Hash.eq(hash.as_slice()))
        .apply_if(hash_type, |q, hash_type| {
            q.filter(Column::HashType.eq(hash_type))
        })
        .apply_if(chain_id, |q, chain_id| {
            q.filter(Column::ChainId.eq(chain_id))
        })
        .cursor_by(Column::ChainId);

    if let Some(page_token) = page_token {
        c.after(page_token);
    }

    paginate_cursor(db, c, page_size, |u| u.chain_id).await
}

pub async fn list_transactions_paginated<C>(
    db: &C,
    hash: BlockHash,
    chain_id: Option<ChainId>,
    page_size: u64,
    page_token: Option<ChainId>,
) -> Result<(Vec<Model>, Option<ChainId>), DbErr>
where
    C: ConnectionTrait,
{
    list_hashes_paginated(
        db,
        hash,
        Some(db_enum::HashType::Transaction),
        chain_id,
        page_size,
        page_token,
    )
    .await
}
