/// Methods intended for interacting with blockscout db
use blockscout_db::entity::blocks;
use chrono::NaiveDateTime;
use sea_orm::{
    sea_query::{self},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Statement,
};
use std::fmt::Debug;

use crate::{data_source::UpdateContext, types::TimespanTrait, ChartError};

pub async fn find_one_value<Value>(
    cx: &UpdateContext<'_>,
    query: Statement,
) -> Result<Option<Value>, ChartError>
where
    Value: FromQueryResult,
{
    Value::find_by_statement(query)
        .one(cx.blockscout)
        .await
        .map_err(ChartError::BlockscoutDB)
}

pub async fn find_all_points<Point>(
    cx: &UpdateContext<'_>,
    statement: Statement,
) -> Result<Vec<Point>, ChartError>
where
    Point: FromQueryResult + TimespanTrait,
    Point::Timespan: Ord,
{
    let find_by_statement = Point::find_by_statement(statement);
    let mut data = find_by_statement
        .all(cx.blockscout)
        .await
        .map_err(ChartError::BlockscoutDB)?;
    // can't use sort_*_by_key: https://github.com/rust-lang/rust/issues/34162
    data.sort_unstable_by(|a, b| a.timespan().cmp(b.timespan()));
    Ok(data)
}

#[derive(FromQueryResult)]
struct MinBlock {
    min_block: i64,
}

pub async fn get_min_block_blockscout(blockscout: &DatabaseConnection) -> Result<i64, DbErr> {
    let min_block = blocks::Entity::find()
        .select_only()
        .column_as(
            sea_query::Expr::col(blocks::Column::Number).min(),
            "min_block",
        )
        .filter(blocks::Column::Consensus.eq(true))
        .into_model::<MinBlock>()
        .one(blockscout)
        .await?;

    min_block
        .map(|r| r.min_block)
        .ok_or_else(|| DbErr::RecordNotFound("no blocks found in blockscout database".into()))
}

#[derive(FromQueryResult)]
struct MinDate {
    timestamp: NaiveDateTime,
}

pub async fn get_min_date_blockscout<C>(blockscout: &C) -> Result<NaiveDateTime, DbErr>
where
    C: ConnectionTrait,
{
    let min_date = blocks::Entity::find()
        .select_only()
        .column(blocks::Column::Timestamp)
        .filter(blocks::Column::Consensus.eq(true))
        // First block on ethereum mainnet has 0 timestamp,
        // however first block on Goerli for example has valid timestamp.
        // Therefore we filter on zero timestamp
        .filter(blocks::Column::Timestamp.ne(NaiveDateTime::default()))
        .order_by_asc(blocks::Column::Number)
        .into_model::<MinDate>()
        .one(blockscout)
        .await?;

    min_date
        .map(|r| r.timestamp)
        .ok_or_else(|| DbErr::RecordNotFound("no blocks found in blockscout database".into()))
}

#[derive(Debug, FromQueryResult)]
struct CountEstimate {
    count: Option<i64>,
}

/// `None` means either that
/// - db hasn't been initialized before
/// - `table_name` wasn't found
pub async fn query_estimated_table_rows(
    blockscout: &DatabaseConnection,
    table_name: &str,
) -> Result<Option<i64>, DbErr> {
    let statement: Statement = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
            SELECT (
                CASE WHEN c.reltuples < 0 THEN
                    NULL
                WHEN c.relpages = 0 THEN
                    float8 '0'
                ELSE c.reltuples / c.relpages
                END *
                (
                    pg_catalog.pg_relation_size(c.oid) /
                    pg_catalog.current_setting('block_size')::int
                )
            )::bigint as count
            FROM pg_catalog.pg_class c
            WHERE c.oid = $1::regclass
        "#,
        vec![table_name.into()],
    );
    let count = CountEstimate::find_by_statement(statement)
        .one(blockscout)
        .await?;
    let count = count.and_then(|c| c.count);
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{
        init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data, point_construction::d,
    };

    use sea_orm::EntityName;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_estimated_table_rows_works() {
        let (_db, blockscout) = init_db_all("get_estimated_table_rows_works").await;
        fill_mock_blockscout_data(&blockscout, d("2023-03-01")).await;

        // need to analyze or vacuum for `reltuples` to be updated.
        // source: https://www.postgresql.org/docs/9.3/planner-stats.html
        let _ = blockscout
            .execute(Statement::from_string(DbBackend::Postgres, "ANALYZE;"))
            .await
            .unwrap();

        let blocks_estimate = query_estimated_table_rows(&blockscout, blocks::Entity.table_name())
            .await
            .unwrap()
            .unwrap();

        // should be 16 rows in the table, but it's an estimate
        assert!(blocks_estimate > 5);
        assert!(blocks_estimate < 30);

        assert!(
            query_estimated_table_rows(
                &blockscout,
                blockscout_db::entity::addresses::Entity.table_name()
            )
            .await
            .unwrap()
            .unwrap()
                > 0
        );

        assert!(
            query_estimated_table_rows(
                &blockscout,
                blockscout_db::entity::transactions::Entity.table_name()
            )
            .await
            .unwrap()
            .unwrap()
                > 0
        );

        assert!(
            query_estimated_table_rows(
                &blockscout,
                blockscout_db::entity::smart_contracts::Entity.table_name()
            )
            .await
            .unwrap()
            .unwrap()
                > 0
        );
    }
}
