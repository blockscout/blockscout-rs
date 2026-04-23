use chrono::{NaiveDateTime, Utc};
use multichain_aggregator_entity::block_ranges;
use num_traits::ToPrimitive;
use rust_decimal::Decimal;
use sea_orm::{
    DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QuerySelect, Statement,
    sea_query::Expr,
};

#[derive(FromQueryResult, Debug)]
struct MinBlock {
    min_block: Option<Decimal>,
}

pub async fn get_min_block_multichain(multichain: &DatabaseConnection) -> Result<i64, DbErr> {
    // sum of all min blocks is a good indicator of new past data
    // because min block for a chain can only decrease as the indexation continues
    // or new chain may be added which will change the sum, indicating a need for the full reupdate
    let value = block_ranges::Entity::find()
        .select_only()
        .column_as(
            Expr::col(block_ranges::Column::MinBlockNumber).sum(),
            "min_block",
        )
        .into_model::<MinBlock>()
        .one(multichain)
        .await?;

    match value.and_then(|r| r.min_block).map(|m| m.to_i64().ok_or(m)) {
        Some(Ok(min_block)) => Ok(min_block),
        Some(Err(sum)) => {
            tracing::warn!(sum =? sum, "failed to convert min block sum to i64");
            Ok(i64::MAX)
        }
        None => {
            tracing::warn!("no block ranges found in multichain database");
            // set max so that if ranges appear, the reupdate is triggered
            Ok(i64::MAX)
        }
    }
}

#[derive(FromQueryResult, Debug)]
struct MinTimestamp {
    min_timestamp: Option<NaiveDateTime>,
}

// Fetching the earliest import date for the cluster’s counters or interop messages.
pub async fn get_min_date_multichain(
    multichain: &DatabaseConnection,
) -> Result<NaiveDateTime, DbErr> {
    let query = r#"
        SELECT MIN(dt) as min_timestamp
        FROM (
            SELECT date::timestamp as dt FROM counters_global_imported
            UNION ALL
            SELECT timestamp as dt FROM interop_messages
        ) t
    "#;

    let result = MinTimestamp::find_by_statement(Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        query.to_owned(),
    ))
    .one(multichain)
    .await?
    .and_then(|r| r.min_timestamp)
    .unwrap_or_else(|| Utc::now().naive_utc());

    Ok(result)
}
