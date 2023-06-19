#![allow(dead_code)]
use blockscout_db::entity::blocks;
use entity::block_ranges;
use sea_orm::{
    prelude::*,
    sea_query::{Expr, OnConflict},
    ConnectionTrait, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("blockscout database error: {0}")]
    BlockscoutDB(DbErr),
    #[error("stats database error: {0}")]
    StatsDB(DbErr),
}

pub async fn from_cache(
    stats: &DatabaseConnection,
    blockscout: &DatabaseConnection,
) -> Result<Vec<block_ranges::Model>, Error> {
    let txn = stats.begin().await.map_err(Error::StatsDB)?;
    let maybe_oldest = block_ranges::Entity::find()
        .order_by_desc(block_ranges::Column::Date)
        .one(&txn)
        .await
        .map_err(Error::StatsDB)?;

    let fetch_new_ranges_query = {
        let date = Expr::cust("date(timestamp)");
        let query = blocks::Entity::find()
            .select_only()
            .column_as(date.clone(), "date")
            .column_as(blocks::Column::Number.min(), "from_number")
            .column_as(blocks::Column::Number.max(), "to_number")
            .filter(blocks::Column::Consensus.eq(true))
            .group_by(date.clone())
            .order_by_asc(date.clone());
        match &maybe_oldest {
            Some(oldest) => query.filter(Expr::expr(date).gte(oldest.date)),
            None => query,
        }
    };
    tracing::info!(maybe_oldest =? maybe_oldest, "start search new block ranges");
    let new_ranges = fetch_new_ranges_query
        .into_model::<block_ranges::Model>()
        .all(blockscout)
        .await
        .map_err(Error::BlockscoutDB)?;
    if !new_ranges.is_empty() {
        insert_ranges(new_ranges.iter(), &txn).await?;
    }
    let all_ranges = block_ranges::Entity::find()
        .order_by_asc(block_ranges::Column::Date)
        .all(stats)
        .await
        .map_err(Error::StatsDB)?;
    txn.commit().await.map_err(Error::StatsDB)?;
    Ok(all_ranges)
}

async fn insert_ranges<C>(
    ranges: impl Iterator<Item = &block_ranges::Model>,
    db: &C,
) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    let to_insert = ranges.into_iter().map(|r| block_ranges::ActiveModel {
        date: Set(r.date),
        from_number: Set(r.from_number),
        to_number: Set(r.to_number),
    });
    block_ranges::Entity::insert_many(to_insert)
        .on_conflict(
            OnConflict::column(block_ranges::Column::Date)
                .update_columns([
                    block_ranges::Column::FromNumber,
                    block_ranges::Column::ToNumber,
                ])
                .to_owned(),
        )
        .exec(db)
        .await
        .map_err(Error::StatsDB)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data};
    use pretty_assertions::assert_eq;

    #[ignore = "needs db"]
    #[tokio::test]
    async fn works() {
        let _ = tracing_subscriber::fmt::try_init();
        let (stats, blockscout) = init_db_all("block_ranges_works", None).await;
        fill_mock_blockscout_data(&blockscout, "2023-12-31").await;
        let ranges: Vec<(String, i64, i64)> = from_cache(&stats, &blockscout)
            .await
            .expect("failed to fetch block ranges")
            .into_iter()
            .map(|r| (r.date.to_string(), r.from_number, r.to_number))
            .collect();
        let expected = [
            ("2022-11-09".into(), 0, 0),
            ("2022-11-10".into(), 1, 3),
            ("2022-11-11".into(), 4, 7),
            ("2022-11-12".into(), 8, 8),
            ("2022-12-01".into(), 9, 9),
            ("2023-01-01".into(), 10, 10),
            ("2023-02-01".into(), 11, 11),
            ("2023-03-01".into(), 12, 12),
        ];
        assert_eq!(ranges.as_slice(), expected);

        // pretend to clear blockscout database (since it doesn't have down migrations)
        let (_, blockscout2) = init_db_all("block_ranges_works_2", None).await;
        let ranges: Vec<(String, i64, i64)> = from_cache(&stats, &blockscout2)
            .await
            .expect("failed to fetch block ranges")
            .into_iter()
            .map(|r| (r.date.to_string(), r.from_number, r.to_number))
            .collect();
        assert_eq!(ranges.as_slice(), expected, "invalid data in cache");
    }
}
