//! Collection of common operations to perform while updating.
//! Can be useful for any chart regardless of their type.

use blockscout_db::entity::blocks;
use chrono::{NaiveDate, NaiveDateTime, Offset};
use entity::{chart_data, charts};
use sea_orm::{
    prelude::*, sea_query, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Set, Unchanged,
};

use crate::{Chart, DateValue, UpdateError};

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
struct SyncInfo {
    pub date: NaiveDate,
    pub value: String,
    pub min_blockscout_block: Option<i64>,
}

/// Get `offset`th last row. Date of the row can be a starting point for an update.
/// Usually used to retrieve last 'finalized' row (for which no recomputations needed).
///
/// Retrieves `offset`th latest data point from DB, if any.
/// In case of inconsistencies or set `force_full`, also returns `None`.
pub async fn get_last_row<C>(
    chart: &C,
    chart_id: i32,
    min_blockscout_block: i64,
    db: &DatabaseConnection,
    force_full: bool,
    offset: Option<u64>,
) -> Result<Option<DateValue>, UpdateError>
where
    C: Chart + ?Sized,
{
    let offset = offset.unwrap_or(0);
    let last_row = if force_full {
        tracing::info!(
            min_blockscout_block = min_blockscout_block,
            chart = chart.name(),
            "running full update due to force override"
        );
        None
    } else {
        let last_row: Option<SyncInfo> = chart_data::Entity::find()
            .column(chart_data::Column::Date)
            .column(chart_data::Column::Value)
            .column(chart_data::Column::MinBlockscoutBlock)
            .filter(chart_data::Column::ChartId.eq(chart_id))
            .order_by_desc(chart_data::Column::Date)
            .offset(offset)
            .into_model()
            .one(db)
            .await
            .map_err(UpdateError::StatsDB)?;

        match last_row {
            Some(row) => {
                if let Some(block) = row.min_blockscout_block {
                    if block == min_blockscout_block {
                        tracing::info!(
                            min_blockscout_block = min_blockscout_block,
                            min_chart_block = block,
                            chart = chart.name(),
                            row = ?row,
                            "running partial update"
                        );
                        Some(DateValue {
                            date: row.date,
                            value: row.value,
                        })
                    } else {
                        tracing::info!(
                            min_blockscout_block = min_blockscout_block,
                            min_chart_block = block,
                            chart = chart.name(),
                            "running full update due to min blocks mismatch"
                        );
                        None
                    }
                } else {
                    tracing::info!(
                        min_blockscout_block = min_blockscout_block,
                        chart = chart.name(),
                        "running full update due to lack of saved min block"
                    );
                    None
                }
            }
            None => {
                tracing::info!(
                    min_blockscout_block = min_blockscout_block,
                    chart = chart.name(),
                    "running full update due to lack of history data"
                );
                None
            }
        }
    };

    Ok(last_row)
}

pub async fn set_last_updated_at<Tz>(
    chart_id: i32,
    db: &DatabaseConnection,
    at: chrono::DateTime<Tz>,
) -> Result<(), DbErr>
where
    Tz: chrono::TimeZone,
{
    let last_updated_at = at.with_timezone(&chrono::Utc.fix());
    let model = charts::ActiveModel {
        id: Unchanged(chart_id),
        last_updated_at: Set(Some(last_updated_at)),
        ..Default::default()
    };
    charts::Entity::update(model)
        .filter(charts::Column::Id.eq(chart_id))
        .exec(db)
        .await?;
    Ok(())
}
