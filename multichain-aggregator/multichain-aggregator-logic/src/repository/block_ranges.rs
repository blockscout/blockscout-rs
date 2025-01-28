use super::paginate_cursor;
use crate::types::{block_ranges::BlockRange, ChainId};
use entity::block_ranges::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    prelude::Expr, sea_query::OnConflict, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DbErr,
    EntityTrait, QueryFilter,
};

pub async fn upsert_many<C>(db: &C, block_ranges: Vec<BlockRange>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    if block_ranges.is_empty() {
        return Ok(());
    }

    let block_ranges = block_ranges.into_iter().map(|block_range| {
        let model: Model = block_range.into();
        let mut active: ActiveModel = model.into();
        active.created_at = NotSet;
        active.updated_at = NotSet;
        active
    });

    Entity::insert_many(block_ranges)
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
        .exec(db)
        .await?;

    Ok(())
}

pub async fn list_matching_block_ranges_paginated<C>(
    db: &C,
    block_number: u64,
    page_size: u64,
    page_token: Option<ChainId>,
) -> Result<(Vec<Model>, Option<ChainId>), DbErr>
where
    C: ConnectionTrait,
{
    let mut c = Entity::find()
        .filter(Column::MinBlockNumber.lte(block_number))
        .filter(Column::MaxBlockNumber.gte(block_number))
        .cursor_by(Column::ChainId);

    if let Some(page_token) = page_token {
        c.after(page_token);
    }

    paginate_cursor(db, c, page_size, |u| u.chain_id).await
}
