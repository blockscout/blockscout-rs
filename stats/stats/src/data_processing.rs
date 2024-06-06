use chrono::NaiveDate;

use crate::{
    charts::db_interaction::types::{DateValue, DateValueString},
    UpdateError,
};
use std::{
    fmt::Display,
    iter::Sum,
    mem,
    ops::{AddAssign, SubAssign},
    str::FromStr,
};

/// `prev_sum` - sum before this data segment
pub fn parse_and_cumsum<T>(
    mut data: Vec<DateValueString>,
    parent_name: &str,
    mut prev_sum: T,
) -> Result<Vec<DateValueString>, UpdateError>
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

/// Allows missing dates in `data`.
/// Assumes `data` is sorted.
///
/// Semantically inverse to [`deltas`].
pub fn cumsum<DV>(mut data: Vec<DV>, mut prev_sum: DV::Value) -> Result<Vec<DV>, UpdateError>
where
    DV: DateValue + Default,
    DV::Value: AddAssign + Clone,
{
    for item in data.iter_mut() {
        let (date, value) = mem::take(item).into_parts();
        prev_sum.add_assign(value);
        *item = DV::from_parts(date, prev_sum.clone());
    }
    Ok(data)
}

/// Allows missing dates in `data`.
/// Assumes `data` is sorted.
///
/// Semantically inverse to [`cumsum`].
pub fn deltas<DV>(mut data: Vec<DV>, mut prev_value: DV::Value) -> Result<Vec<DV>, UpdateError>
where
    DV: DateValue + Default,
    DV::Value: SubAssign + Clone,
{
    for item in data.iter_mut() {
        let (date, mut value) = mem::take(item).into_parts();
        let prev_copy = prev_value.clone();
        prev_value = value.clone();
        value -= prev_copy;
        *item = DV::from_parts(date, value);
    }
    Ok(data)
}

pub fn parse_and_sum<T>(
    data: Vec<DateValueString>,
    chart_name: &str,
    parent_name: &str,
) -> Result<Option<DateValueString>, UpdateError>
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
    let point = DateValueString {
        date: max_date,
        value: total.to_string(),
    };
    Ok(Some(point))
}

pub fn sum<DV>(data: &[DV], mut partial_sum: DV::Value) -> Result<DV, UpdateError>
where
    DV: DateValue + Default,
    DV::Value: AddAssign + Clone,
{
    let mut max_date = NaiveDate::MIN;
    for item in data.iter() {
        let (date, value) = item.get_parts();
        partial_sum.add_assign(value.clone());
        max_date = max_date.max(*date);
    }
    let sum_point = DV::from_parts(max_date, partial_sum);
    Ok(sum_point)
}

pub fn last_point(data: Vec<DateValueString>) -> Option<DateValueString> {
    data.into_iter().max()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::tests::point_construction::{v_decimal, v_int};

    #[test]
    fn test_deltas_works_int() {
        let test_cases = [
            // Empty vector
            (vec![], 0, vec![]),
            // Normal case: 3 sequential dates with increments
            (
                vec![
                    v_int("2100-01-01", 100),
                    v_int("2100-01-02", 110),
                    v_int("2100-01-03", 120),
                ],
                90,
                vec![
                    v_int("2100-01-01", 10),
                    v_int("2100-01-02", 10),
                    v_int("2100-01-03", 10),
                ],
            ),
            // Increments and decrements with integer values
            (
                vec![
                    v_int("2100-01-01", 100),
                    v_int("2100-01-02", 110),
                    v_int("2100-01-03", 90),
                ],
                95,
                vec![
                    v_int("2100-01-01", 5),
                    v_int("2100-01-02", 10),
                    v_int("2100-01-03", -20),
                ],
            ),
            // Missing dates with integer values
            (
                vec![
                    v_int("2100-01-01", 100),
                    v_int("2100-01-05", 150),
                    v_int("2100-01-10", 130),
                ],
                95,
                vec![
                    v_int("2100-01-01", 5),
                    v_int("2100-01-05", 50),
                    v_int("2100-01-10", -20),
                ],
            ),
        ];

        for (data, prev_value, expected) in test_cases {
            assert_eq!(deltas(data, prev_value).unwrap(), expected);
        }
    }

    #[test]
    fn test_deltas_works_decimal() {
        let test_cases = [
            // Empty vector
            (vec![], dec!(0.0), vec![]),
            // Normal case: 3 sequential dates with increments
            (
                vec![
                    v_decimal("2100-01-01", dec!(100.5)),
                    v_decimal("2100-01-02", dec!(110.75)),
                    v_decimal("2100-01-03", dec!(120.25)),
                ],
                dec!(90.0),
                vec![
                    v_decimal("2100-01-01", dec!(10.5)),
                    v_decimal("2100-01-02", dec!(10.25)),
                    v_decimal("2100-01-03", dec!(9.5)),
                ],
            ),
            // Increments and decrements with decimal values
            (
                vec![
                    v_decimal("2100-01-01", dec!(100.5)),
                    v_decimal("2100-01-02", dec!(110.75)),
                    v_decimal("2100-01-03", dec!(90.25)),
                ],
                dec!(95.0),
                vec![
                    v_decimal("2100-01-01", dec!(5.5)),
                    v_decimal("2100-01-02", dec!(10.25)),
                    v_decimal("2100-01-03", dec!(-20.5)),
                ],
            ),
            // Missing dates with decimal values
            (
                vec![
                    v_decimal("2100-01-01", dec!(100.5)),
                    v_decimal("2100-01-05", dec!(150.25)),
                    v_decimal("2100-01-10", dec!(130.75)),
                ],
                dec!(95.0),
                vec![
                    v_decimal("2100-01-01", dec!(5.5)),
                    v_decimal("2100-01-05", dec!(49.75)),
                    v_decimal("2100-01-10", dec!(-19.5)),
                ],
            ),
        ];

        for (data, prev_value, expected) in test_cases {
            assert_eq!(deltas(data, prev_value).unwrap(), expected);
        }
    }

    #[test]
    fn test_cumsum_works_int() {
        let test_cases = [
            // Empty vector
            (vec![], 0, vec![]),
            // Normal case: 3 sequential dates with increments
            (
                vec![
                    v_int("2100-01-01", 10),
                    v_int("2100-01-02", 20),
                    v_int("2100-01-03", 30),
                ],
                100,
                vec![
                    v_int("2100-01-01", 110),
                    v_int("2100-01-02", 130),
                    v_int("2100-01-03", 160),
                ],
            ),
            // Increments and decrements with integer values
            (
                vec![
                    v_int("2100-01-01", 0),
                    v_int("2100-01-02", -10),
                    v_int("2100-01-03", 20),
                ],
                100,
                vec![
                    v_int("2100-01-01", 100),
                    v_int("2100-01-02", 90),
                    v_int("2100-01-03", 110),
                ],
            ),
            // Missing dates with integer values
            (
                vec![
                    v_int("2100-01-01", 10),
                    v_int("2100-01-05", 20),
                    v_int("2100-01-10", 30),
                ],
                100,
                vec![
                    v_int("2100-01-01", 110),
                    v_int("2100-01-05", 130),
                    v_int("2100-01-10", 160),
                ],
            ),
        ];

        for (data, prev_value, expected) in test_cases {
            assert_eq!(cumsum(data, prev_value).unwrap(), expected);
        }
    }

    #[test]
    fn test_cumsum_works_decimal() {
        let test_cases = [
            // Empty vector
            (vec![], dec!(0.0), vec![]),
            // Normal case: 3 sequential dates with increments
            (
                vec![
                    v_decimal("2100-01-01", dec!(100.5)),
                    v_decimal("2100-01-02", dec!(110.75)),
                    v_decimal("2100-01-03", dec!(120.25)),
                ],
                dec!(100.0),
                vec![
                    v_decimal("2100-01-01", dec!(200.5)),
                    v_decimal("2100-01-02", dec!(311.25)),
                    v_decimal("2100-01-03", dec!(431.5)),
                ],
            ),
            // Increments and decrements with decimal values
            (
                vec![
                    v_decimal("2100-01-01", dec!(100.5)),
                    v_decimal("2100-01-02", dec!(110.75)),
                    v_decimal("2100-01-03", dec!(90.25)),
                ],
                dec!(0.0),
                vec![
                    v_decimal("2100-01-01", dec!(100.5)),
                    v_decimal("2100-01-02", dec!(211.25)),
                    v_decimal("2100-01-03", dec!(301.5)),
                ],
            ),
            // Missing dates with decimal values
            (
                vec![
                    v_decimal("2100-01-01", dec!(100.5)),
                    v_decimal("2100-01-05", dec!(150.25)),
                    v_decimal("2100-01-10", dec!(130.75)),
                ],
                dec!(1.0),
                vec![
                    v_decimal("2100-01-01", dec!(101.5)),
                    v_decimal("2100-01-05", dec!(251.75)),
                    v_decimal("2100-01-10", dec!(382.5)),
                ],
            ),
        ];

        for (data, initial_value, expected) in test_cases {
            assert_eq!(cumsum(data, initial_value).unwrap(), expected);
        }
    }
}
