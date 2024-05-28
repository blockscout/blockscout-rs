use crate::{charts::db_interaction::types::DateValue, UpdateError};
use std::{fmt::Display, iter::Sum, ops::AddAssign, str::FromStr};

/// `prev_sum` - sum before this data segment
pub fn parse_and_cumsum<T>(
    mut data: Vec<DateValue>,
    parent_name: &str,
    mut prev_sum: T,
) -> Result<Vec<DateValue>, UpdateError>
where
    T: AddAssign + FromStr + Default + Display,
    T::Err: Display,
{
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
