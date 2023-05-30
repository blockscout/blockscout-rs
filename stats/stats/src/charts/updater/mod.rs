use blockscout_db::entity::blocks;
use chrono::{NaiveDate, NaiveDateTime};
use entity::chart_data;
use sea_orm::{prelude::*, sea_query, ConnectionTrait, FromQueryResult, QueryOrder, QuerySelect};
mod batch;
mod dependent;
mod full;
mod partial;

pub use batch::ChartBatchUpdater;
pub use dependent::{last_point, parse_and_growth, parse_and_sum, ChartDependentUpdater};
pub use full::ChartFullUpdater;
pub use partial::ChartPartialUpdater;

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

pub async fn get_last_row<C>(
    chart: &C,
    chart_id: i32,
    min_blockscout_block: i64,
    db: &DatabaseConnection,
    force_full: bool,
) -> Result<Option<DateValue>, UpdateError>
where
    C: Chart + ?Sized,
{
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
            .offset(1)
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
