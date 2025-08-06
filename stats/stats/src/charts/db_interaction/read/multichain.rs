use chrono::NaiveDateTime;
use multichain_aggregator_entity::block_ranges;
use multichain_aggregator_entity::counters_global_imported;
use num_traits::ToPrimitive;
use rust_decimal::Decimal;
use chrono::{NaiveDate, Utc};
use sea_orm::{
    DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QuerySelect, sea_query::Expr,
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
struct MinDate {
    date: Option<NaiveDate>,
}

// Getting the earliest date when the cluster's counters are available  
pub async fn get_min_date_multichain(multichain: &DatabaseConnection) -> Result<NaiveDateTime, DbErr> {
    let min_date = counters_global_imported::Entity::find()
        .select_only()
        .column_as(Expr::col(counters_global_imported::Column::Date).min(), "date")
        .into_model::<MinDate>()
        .one(multichain)
        .await?;

    let naive_date = min_date
        .and_then(|r| r.date)
        .unwrap_or_else(|| Utc::now().date_naive());

    let naive_datetime = naive_date.and_hms_opt(0, 0, 0)
        .ok_or_else(|| DbErr::Custom("Invalid time: 00:00:00".into()))?;

    Ok(naive_datetime)
}