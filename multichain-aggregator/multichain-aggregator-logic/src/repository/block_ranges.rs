use crate::{
    error::ServiceError,
    types::block_ranges::{BlockRange, ChainBlockNumber},
};
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

pub async fn find_matching_block_ranges<C>(
    db: &C,
    block_number: u64,
) -> Result<Vec<BlockRange>, DbErr>
where
    C: ConnectionTrait,
{
    let res = Entity::find()
        .filter(Column::MinBlockNumber.lte(block_number))
        .filter(Column::MaxBlockNumber.gte(block_number))
        .all(db)
        .await?
        .into_iter()
        .map(|r| r.into())
        .collect();
    Ok(res)
}

pub async fn search_by_query<C>(db: &C, query: &str) -> Result<Vec<ChainBlockNumber>, ServiceError>
where
    C: ConnectionTrait,
{
    let block_number = match query.parse() {
        Ok(block_number) => block_number,
        Err(_) => return Ok(vec![]),
    };

    Ok(find_matching_block_ranges(db, block_number)
        .await?
        .into_iter()
        .map(|r| ChainBlockNumber {
            chain_id: r.chain_id,
            block_number,
        })
        .collect())
}
