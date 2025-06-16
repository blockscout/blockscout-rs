use super::paginate_cursor;
use crate::types::{block_ranges::BlockRange, ChainId};
use entity::block_ranges::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    prelude::Expr, sea_query::OnConflict, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DbErr,
    EntityTrait, QueryFilter, QueryTrait,
};

pub async fn upsert_many<C>(db: &C, block_ranges: Vec<BlockRange>) -> Result<Vec<Model>, DbErr>
where
    C: ConnectionTrait,
{
    if block_ranges.is_empty() {
        return Ok(vec![]);
    }

    let block_ranges = block_ranges.into_iter().map(|block_range| {
        let model: Model = block_range.into();
        let mut active: ActiveModel = model.into();
        active.created_at = NotSet;
        active.updated_at = NotSet;
        active
    });

    let models = Entity::insert_many(block_ranges)
        .on_conflict(
            OnConflict::column(Column::ChainId)
                .values([
                    (Column::UpdatedAt, Expr::current_timestamp().into()),
                    (
                        Column::MinBlockNumber,
                        Expr::cust_with_exprs(
                            "LEAST($1, $2)",
                            [
                                Column::MinBlockNumber.into_expr().into(),
                                Expr::cust("EXCLUDED.min_block_number"),
                            ],
                        ),
                    ),
                    (
                        Column::MaxBlockNumber,
                        Expr::cust_with_exprs(
                            "GREATEST($1, $2)",
                            [
                                Column::MaxBlockNumber.into_expr().into(),
                                Expr::cust("EXCLUDED.max_block_number"),
                            ],
                        ),
                    ),
                ])
                .to_owned(),
        )
        .exec_with_returning_many(db)
        .await?;

    Ok(models)
}

pub async fn list_matching_block_ranges_paginated<C>(
    db: &C,
    block_number: u64,
    chain_ids: Vec<ChainId>,
    page_size: u64,
    page_token: Option<ChainId>,
) -> Result<(Vec<Model>, Option<ChainId>), DbErr>
where
    C: ConnectionTrait,
{
    let c = Entity::find()
        .apply_if(
            (!chain_ids.is_empty()).then_some(chain_ids),
            |q, chain_ids| q.filter(Column::ChainId.is_in(chain_ids)),
        )
        .filter(Column::MinBlockNumber.lte(block_number))
        .filter(Column::MaxBlockNumber.gte(block_number))
        .cursor_by(Column::ChainId);

    paginate_cursor(db, c, page_size, page_token, |u| u.chain_id).await
}
