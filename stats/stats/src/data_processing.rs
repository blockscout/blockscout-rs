use chrono::NaiveDate;

use crate::{
    charts::types::timespans::DateValue,
    types::{Timespan, TimespanValue},
    UpdateError,
};
use std::{
    mem,
    ops::{AddAssign, SubAssign},
};

/// Allows missing dates in `data`.
/// Assumes `data` is sorted.
///
/// Semantically inverse to [`deltas`].
pub fn cumsum<Resolution, Value>(
    mut data: Vec<TimespanValue<Resolution, Value>>,
    mut prev_sum: Value,
) -> Result<Vec<TimespanValue<Resolution, Value>>, UpdateError>
where
    Value: AddAssign + Clone,
    TimespanValue<Resolution, Value>: Default,
{
    for item in data.iter_mut() {
        let TimespanValue { timespan, value } = mem::take(item);
        prev_sum.add_assign(value);
        *item = TimespanValue::<Resolution, Value> {
            timespan,
            value: prev_sum.clone(),
        };
    }
    Ok(data)
}

/// Allows missing dates in `data`.
/// Assumes `data` is sorted.
///
/// Semantically inverse to [`cumsum`].
pub fn deltas<Resolution, Value>(
    mut data: Vec<TimespanValue<Resolution, Value>>,
    mut prev_value: Value,
) -> Result<Vec<TimespanValue<Resolution, Value>>, UpdateError>
where
    Value: SubAssign + Clone,
    TimespanValue<Resolution, Value>: Default,
{
    for item in data.iter_mut() {
        let TimespanValue {
            timespan,
            mut value,
        } = mem::take(item);
        let prev_copy = prev_value.clone();
        prev_value = value.clone();
        value -= prev_copy;
        *item = TimespanValue::<Resolution, Value> { timespan, value };
    }
    Ok(data)
}

pub fn sum<Resolution, Value>(
    data: &[TimespanValue<Resolution, Value>],
    mut partial_sum: Value,
) -> Result<TimespanValue<Resolution, Value>, UpdateError>
where
    Resolution: Timespan + Clone + Ord,
    Value: AddAssign + Clone,
{
    let mut max_timespan = Resolution::from_date(NaiveDate::MIN);
    for item in data.iter() {
        let TimespanValue { timespan, value } = item;
        partial_sum.add_assign(value.clone());
        max_timespan = max_timespan.max(timespan.clone());
    }
    let sum_point = TimespanValue::<Resolution, Value> {
        timespan: max_timespan,
        value: partial_sum,
    };
    Ok(sum_point)
}

pub fn last_point(data: Vec<DateValue<String>>) -> Option<DateValue<String>> {
    data.into_iter().max()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::tests::point_construction::{d_v_decimal, d_v_int};

    #[test]
    fn test_deltas_works_int() {
        let test_cases = [
            // Empty vector
            (vec![], 0, vec![]),
            // Normal case: 3 sequential dates with increments
            (
                vec![
                    d_v_int("2100-01-01", 100),
                    d_v_int("2100-01-02", 110),
                    d_v_int("2100-01-03", 120),
                ],
                90,
                vec![
                    d_v_int("2100-01-01", 10),
                    d_v_int("2100-01-02", 10),
                    d_v_int("2100-01-03", 10),
                ],
            ),
            // Increments and decrements with integer values
            (
                vec![
                    d_v_int("2100-01-01", 100),
                    d_v_int("2100-01-02", 110),
                    d_v_int("2100-01-03", 90),
                ],
                95,
                vec![
                    d_v_int("2100-01-01", 5),
                    d_v_int("2100-01-02", 10),
                    d_v_int("2100-01-03", -20),
                ],
            ),
            // Missing dates with integer values
            (
                vec![
                    d_v_int("2100-01-01", 100),
                    d_v_int("2100-01-05", 150),
                    d_v_int("2100-01-10", 130),
                ],
                95,
                vec![
                    d_v_int("2100-01-01", 5),
                    d_v_int("2100-01-05", 50),
                    d_v_int("2100-01-10", -20),
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
                    d_v_decimal("2100-01-01", dec!(100.5)),
                    d_v_decimal("2100-01-02", dec!(110.75)),
                    d_v_decimal("2100-01-03", dec!(120.25)),
                ],
                dec!(90.0),
                vec![
                    d_v_decimal("2100-01-01", dec!(10.5)),
                    d_v_decimal("2100-01-02", dec!(10.25)),
                    d_v_decimal("2100-01-03", dec!(9.5)),
                ],
            ),
            // Increments and decrements with decimal values
            (
                vec![
                    d_v_decimal("2100-01-01", dec!(100.5)),
                    d_v_decimal("2100-01-02", dec!(110.75)),
                    d_v_decimal("2100-01-03", dec!(90.25)),
                ],
                dec!(95.0),
                vec![
                    d_v_decimal("2100-01-01", dec!(5.5)),
                    d_v_decimal("2100-01-02", dec!(10.25)),
                    d_v_decimal("2100-01-03", dec!(-20.5)),
                ],
            ),
            // Missing dates with decimal values
            (
                vec![
                    d_v_decimal("2100-01-01", dec!(100.5)),
                    d_v_decimal("2100-01-05", dec!(150.25)),
                    d_v_decimal("2100-01-10", dec!(130.75)),
                ],
                dec!(95.0),
                vec![
                    d_v_decimal("2100-01-01", dec!(5.5)),
                    d_v_decimal("2100-01-05", dec!(49.75)),
                    d_v_decimal("2100-01-10", dec!(-19.5)),
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
                    d_v_int("2100-01-01", 10),
                    d_v_int("2100-01-02", 20),
                    d_v_int("2100-01-03", 30),
                ],
                100,
                vec![
                    d_v_int("2100-01-01", 110),
                    d_v_int("2100-01-02", 130),
                    d_v_int("2100-01-03", 160),
                ],
            ),
            // Increments and decrements with integer values
            (
                vec![
                    d_v_int("2100-01-01", 0),
                    d_v_int("2100-01-02", -10),
                    d_v_int("2100-01-03", 20),
                ],
                100,
                vec![
                    d_v_int("2100-01-01", 100),
                    d_v_int("2100-01-02", 90),
                    d_v_int("2100-01-03", 110),
                ],
            ),
            // Missing dates with integer values
            (
                vec![
                    d_v_int("2100-01-01", 10),
                    d_v_int("2100-01-05", 20),
                    d_v_int("2100-01-10", 30),
                ],
                100,
                vec![
                    d_v_int("2100-01-01", 110),
                    d_v_int("2100-01-05", 130),
                    d_v_int("2100-01-10", 160),
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
                    d_v_decimal("2100-01-01", dec!(100.5)),
                    d_v_decimal("2100-01-02", dec!(110.75)),
                    d_v_decimal("2100-01-03", dec!(120.25)),
                ],
                dec!(100.0),
                vec![
                    d_v_decimal("2100-01-01", dec!(200.5)),
                    d_v_decimal("2100-01-02", dec!(311.25)),
                    d_v_decimal("2100-01-03", dec!(431.5)),
                ],
            ),
            // Increments and decrements with decimal values
            (
                vec![
                    d_v_decimal("2100-01-01", dec!(100.5)),
                    d_v_decimal("2100-01-02", dec!(110.75)),
                    d_v_decimal("2100-01-03", dec!(90.25)),
                ],
                dec!(0.0),
                vec![
                    d_v_decimal("2100-01-01", dec!(100.5)),
                    d_v_decimal("2100-01-02", dec!(211.25)),
                    d_v_decimal("2100-01-03", dec!(301.5)),
                ],
            ),
            // Missing dates with decimal values
            (
                vec![
                    d_v_decimal("2100-01-01", dec!(100.5)),
                    d_v_decimal("2100-01-05", dec!(150.25)),
                    d_v_decimal("2100-01-10", dec!(130.75)),
                ],
                dec!(1.0),
                vec![
                    d_v_decimal("2100-01-01", dec!(101.5)),
                    d_v_decimal("2100-01-05", dec!(251.75)),
                    d_v_decimal("2100-01-10", dec!(382.5)),
                ],
            ),
        ];

        for (data, initial_value, expected) in test_cases {
            assert_eq!(cumsum(data, initial_value).unwrap(), expected);
        }
    }
}
