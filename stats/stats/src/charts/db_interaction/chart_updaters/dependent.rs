//! Updates chart according to data from another chart.
//! I.e. current chart depends on another (on "parent")

use super::{common_operations::get_min_block_blockscout, ChartUpdater};
use crate::{
    charts::{
        db_interaction::{types::DateValue, write::insert_data_many},
        find_chart,
    },
    get_chart_data, UpdateError,
};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::prelude::*;
use std::{fmt::Display, iter::Sum, ops::AddAssign, str::FromStr, sync::Arc};

#[async_trait]
pub trait ChartDependentUpdater<P>: ChartUpdater
where
    P: ChartUpdater + Send,
{
    fn parent(&self) -> Arc<P>;

    // Note that usually this chart's `approximate_trailing_points` logically
    // matches the one of it's parent
    fn parent_approximate_trailing_points(&self) -> u64 {
        self.parent().approximate_trailing_points()
    }

    async fn get_values(&self, parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError>;

    async fn get_parent_data(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        current_time: chrono::DateTime<Utc>,
        force_full: bool,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let parent = self.parent();
        tracing::info!(
            chart_name = self.name(),
            parent_chart_name = parent.name(),
            "updating parent"
        );
        parent
            .update_with_mutex(db, blockscout, current_time, force_full)
            .await?;
        let data = get_chart_data(
            db,
            parent.name(),
            None,
            None,
            None,
            None,
            self.parent_approximate_trailing_points(),
        )
        .await?;
        let data = data.into_iter().map(DateValue::from).collect();
        Ok(data)
    }

    async fn update_with_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        current_time: chrono::DateTime<Utc>,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let parent_data = self
            .get_parent_data(db, blockscout, current_time, force_full)
            .await?;
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

pub fn parse_and_cumsum<T>(
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
) -> Result<Option<DateValue>, UpdateError>
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
            return Ok(None);
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
    Ok(Some(point))
}

pub fn last_point(data: Vec<DateValue>) -> Option<DateValue> {
    data.into_iter().max()
}
