use super::{
    find_chart,
    insert::{insert_data_many, DateValue},
};
use crate::{get_chart_data, metrics, Chart, UpdateError};
use async_trait::async_trait;
use blockscout_db::entity::blocks;
use chrono::NaiveDate;
use entity::chart_data;
use sea_orm::{prelude::*, sea_query, FromQueryResult, QueryOrder, QuerySelect};
use std::{fmt::Display, iter::Sum, ops::AddAssign, str::FromStr, sync::Arc};

pub fn parse_and_growth<T>(
    mut data: Vec<DateValue>,
    parent_name: &str,
) -> Result<Vec<DateValue>, UpdateError>
where
    T: AddAssign + FromStr + Default + Display,
    T::Err: Display,
{
    let mut prev_sum = T::default();
    for item in data.iter_mut() {
        let value = item.value.parse::<T>().map_err(|e| {
            UpdateError::Internal(format!(
                "failed to parse values in chart '{parent_name}': {e}",
            ))
        })?;
        prev_sum += value;
        item.value = prev_sum.to_string();
    }
    Ok(data)
}

pub fn parse_and_sum<T>(
    data: Vec<DateValue>,
    chart_name: &str,
    parent_name: &str,
) -> Result<Vec<DateValue>, UpdateError>
where
    T: Sum + FromStr + Default + Display,
    T::Err: Display,
{
    let max_date = match data.iter().max() {
        Some(max_date) => max_date.date,
        None => {
            tracing::warn!(
                chart_name = chart_name,
                parent_chart_name = parent_name,
                "parent doesn't have any data after update"
            );
            return Ok(vec![]);
        }
    };
    let total: T = data
        .into_iter()
        .map(|p| p.value.parse::<T>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            UpdateError::Internal(format!(
                "failed to parse values in chart '{parent_name}': {e}",
            ))
        })?
        .into_iter()
        .sum();
    let point = DateValue {
        date: max_date,
        value: total.to_string(),
    };
    Ok(vec![point])
}

#[async_trait]
pub trait ChartFullUpdater: Chart {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError>;

    async fn update_with_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        _force_full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = super::find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let values = {
            let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[self.name()])
                .start_timer();
            self.get_values(blockscout)
                .await?
                .into_iter()
                .map(|value| value.active_model(chart_id, None))
        };
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        Ok(())
    }
}

#[derive(Debug, FromQueryResult)]
struct SyncInfo {
    pub date: NaiveDate,
    pub value: String,
    pub min_blockscout_block: Option<i64>,
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

#[async_trait]
pub trait ChartUpdater: Chart {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError>;

    async fn update_with_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = super::find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let last_row = get_last_row(self, chart_id, min_blockscout_block, db, force_full).await?;
        let values = {
            let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[self.name()])
                .start_timer();
            self.get_values(blockscout, last_row)
                .await?
                .into_iter()
                .map(|value| value.active_model(chart_id, Some(min_blockscout_block)))
        };
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}

#[async_trait]
pub trait ChartDependentUpdate<P>: Chart
where
    P: Chart + Send,
{
    fn parent(&self) -> Arc<P>;

    async fn get_values(&self, parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError>;

    async fn get_parent_data(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let parent = self.parent();
        tracing::info!(
            chart_name = self.name(),
            parent_chart_name = parent.name(),
            "updating parent"
        );
        parent.update(db, blockscout, force_full).await?;
        let data = get_chart_data(db, parent.name(), None, None).await?;
        Ok(data)
    }

    async fn update_with_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let parent_data = self.get_parent_data(db, blockscout, force_full).await?;
        let values = self
            .get_values(parent_data)
            .await?
            .into_iter()
            .map(|v| v.active_model(chart_id, Some(min_blockscout_block)));
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)
    }
}

async fn get_last_row<C>(
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
