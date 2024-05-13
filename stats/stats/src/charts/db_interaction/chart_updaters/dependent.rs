//! Updates chart according to data from another chart.
//! I.e. current chart depends on another (on "parent")

use super::{common_operations::get_min_block_blockscout, ChartUpdater};
use crate::{
    charts::{
        db_interaction::{types::DateValue, write::insert_data_many},
        find_chart,
    },
    data_source::types::{UpdateContext, UpdateParameters},
    get_chart_data, UpdateError,
};
use std::{fmt::Display, iter::Sum, ops::AddAssign, str::FromStr};

pub trait ChartDependentUpdater<P>: ChartUpdater
where
    P: ChartUpdater + Send,
{
    // Note that usually this chart's `approximate_trailing_points` logically
    // matches the one of it's parent
    fn parent_approximate_trailing_points() -> u64 {
        P::approximate_trailing_points()
    }

    async fn get_values(parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError>;

    async fn get_parent_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        tracing::info!(
            chart_name = Self::name(),
            parent_chart_name = P::name(),
            "updating parent"
        );
        P::update_with_mutex(cx).await?;
        let data = get_chart_data(
            cx.user_context.db,
            P::name(),
            None,
            None,
            None,
            None,
            Self::parent_approximate_trailing_points(),
        )
        .await?;
        let data = data.into_iter().map(DateValue::from).collect();
        Ok(data)
    }

    async fn update_with_values(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let chart_id = find_chart(cx.user_context.db, Self::name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(cx.user_context.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let parent_data = Self::get_parent_data(cx).await?;
        let data = Self::get_values(parent_data).await?;
        let values = data
            .clone()
            .into_iter()
            .map(|v| v.active_model(chart_id, Some(min_blockscout_block)));
        insert_data_many(cx.user_context.db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(data)
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
