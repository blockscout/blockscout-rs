//! Tools for operating with missing data
use crate::{DateValueString, MissingDatePolicy, ReadError};
use chrono::{Duration, NaiveDate};

/// Fills missing points according to policy and filters out points outside of range.
///
/// Note that values outside of the range can still affect the filled values.
pub fn fill_and_filter_chart(
    data: Vec<DateValueString>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    policy: MissingDatePolicy,
    interval_limit: Option<Duration>,
) -> Result<Vec<DateValueString>, ReadError> {
    let retrieved_count = data.len();
    let data_filled = fill_missing_points(data, policy, from, to, interval_limit)?;
    if let Some(filled_count) = data_filled.len().checked_sub(retrieved_count) {
        if filled_count > 0 {
            tracing::debug!(policy = ?policy, "{} missing points were filled", filled_count);
        }
    }
    let filled_len = data_filled.len();
    let data_filtered = filter_within_range(data_filled, from, to);
    if let Some(filtered) = filled_len.checked_sub(data_filtered.len()) {
        if filtered > 0 {
            tracing::debug!(range = ?(from, to), "{} points outside of range were removed", filtered);
        }
    }
    Ok(data_filtered)
}

/// Fills values for all dates from `min(data.first(), from)` to `max(data.last(), to)` according
/// to `policy`.
///
/// See [`filled_zeros_data`] and [`filled_previous_data`] for details on the policies.
pub fn fill_missing_points(
    data: Vec<DateValueString>,
    policy: MissingDatePolicy,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    interval_limit: Option<Duration>,
) -> Result<Vec<DateValueString>, ReadError> {
    let from = vec![from.as_ref(), data.first().map(|v| &v.date)]
        .into_iter()
        .flatten()
        .min();
    let to = vec![to.as_ref(), data.last().map(|v| &v.date)]
        .into_iter()
        .flatten()
        .max();
    let (from, to) = match (from, to) {
        (Some(from), Some(to)) if from <= to => (from.to_owned(), to.to_owned()),
        // data is empty or ill-formed
        _ => return Ok(data),
    };

    if let Some(interval_limit) = interval_limit {
        if to - from > interval_limit {
            return Err(ReadError::IntervalLimitExceeded(interval_limit));
        }
    }

    Ok(match policy {
        MissingDatePolicy::FillZero => filled_zeros_data(&data, from, to),
        MissingDatePolicy::FillPrevious => filled_previous_data(&data, from, to),
    })
}

/// Inserts zero values in `data` for all missing dates in inclusive range `[from; to]`
fn filled_zeros_data(
    data: &[DateValueString],
    from: NaiveDate,
    to: NaiveDate,
) -> Vec<DateValueString> {
    let n = (to - from).num_days() as usize;
    let mut new_data: Vec<DateValueString> = Vec::with_capacity(n);

    let mut current_date = from;
    let mut i = 0;
    while current_date <= to {
        let maybe_value_exists = data.get(i).filter(|&v| v.date.eq(&current_date));

        let value = match maybe_value_exists {
            Some(value) => {
                i += 1;
                value.clone()
            }
            None => DateValueString::zero(current_date),
        };
        new_data.push(value);
        current_date += Duration::days(1);
    }

    new_data
}

/// Inserts last existing values in `data` for all missing dates in inclusive range `[from; to]`.
/// For all leading missing dates inserts zero.
fn filled_previous_data(
    data: &[DateValueString],
    from: NaiveDate,
    to: NaiveDate,
) -> Vec<DateValueString> {
    let n = (to - from).num_days() as usize;
    let mut new_data: Vec<DateValueString> = Vec::with_capacity(n);
    let mut current_date = from;
    let mut i = 0;
    while current_date <= to {
        let maybe_value_exists = data.get(i).filter(|&v| v.date.eq(&current_date));
        let value = match maybe_value_exists {
            Some(value) => {
                i += 1;
                value.clone()
            }
            None => new_data
                .last()
                .map(|value| DateValueString {
                    date: current_date,
                    value: value.value.clone(),
                })
                .unwrap_or_else(|| DateValueString::zero(current_date)),
        };
        new_data.push(value);
        current_date += Duration::days(1);
    }
    new_data
}

pub(crate) fn filter_within_range(
    data: Vec<DateValueString>,
    maybe_from: Option<NaiveDate>,
    maybe_to: Option<NaiveDate>,
) -> Vec<DateValueString> {
    let is_within_range = |v: &DateValueString| -> bool {
        if let Some(from) = maybe_from {
            if v.date < from {
                return false;
            }
        }
        if let Some(to) = maybe_to {
            if v.date > to {
                return false;
            }
        }
        true
    };

    data.into_iter().filter(is_within_range).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    fn d(date: &str) -> NaiveDate {
        date.parse().unwrap()
    }
    fn v(date: &str, value: &str) -> DateValueString {
        DateValueString {
            date: d(date),
            value: value.to_string(),
        }
    }

    #[test]
    fn fill_zeros_works() {
        for (data, expected, from, to) in [
            (vec![], vec![], None, None),
            (vec![], vec![], Some(d("2100-01-01")), None),
            (
                vec![],
                vec![v("2100-01-01", "0")],
                Some(d("2100-01-01")),
                Some(d("2100-01-01")),
            ),
            (
                vec![v("2022-01-01", "01")],
                vec![v("2022-01-01", "01")],
                Some(d("2022-01-01")),
                Some(d("2022-01-01")),
            ),
            (
                vec![v("2022-01-01", "01"), v("2022-01-02", "02")],
                vec![v("2022-01-01", "01"), v("2022-01-02", "02")],
                Some(d("2022-01-01")),
                Some(d("2022-01-02")),
            ),
            (
                vec![v("2022-01-01", "01")],
                vec![v("2022-01-01", "01"), v("2022-01-02", "0")],
                Some(d("2022-01-01")),
                Some(d("2022-01-02")),
            ),
            (
                vec![v("2022-01-02", "02")],
                vec![v("2022-01-01", "0"), v("2022-01-02", "02")],
                Some(d("2022-01-01")),
                Some(d("2022-01-02")),
            ),
            (
                vec![
                    v("2022-08-20", "20"),
                    v("2022-08-22", "22"),
                    v("2022-08-23", "23"),
                    v("2022-08-25", "25"),
                ],
                vec![
                    v("2022-08-18", "0"),
                    v("2022-08-19", "0"),
                    v("2022-08-20", "20"),
                    v("2022-08-21", "0"),
                    v("2022-08-22", "22"),
                    v("2022-08-23", "23"),
                    v("2022-08-24", "0"),
                    v("2022-08-25", "25"),
                    v("2022-08-26", "0"),
                    v("2022-08-27", "0"),
                ],
                Some(d("2022-08-18")),
                Some(d("2022-08-27")),
            ),
            (
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-15", "12"),
                ],
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-11", "0"),
                    v("2023-07-12", "12"),
                    v("2023-07-13", "0"),
                    v("2023-07-14", "0"),
                    v("2023-07-15", "12"),
                ],
                Some(d("2023-07-12")),
                Some(d("2023-07-14")),
            ),
            (
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-15", "12"),
                ],
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-11", "0"),
                    v("2023-07-12", "12"),
                    v("2023-07-13", "0"),
                    v("2023-07-14", "0"),
                    v("2023-07-15", "12"),
                ],
                Some(d("2023-07-12")),
                Some(d("2023-07-13")),
            ),
        ] {
            let actual =
                fill_missing_points(data, MissingDatePolicy::FillZero, from, to, None).unwrap();
            assert_eq!(expected, actual)
        }
    }

    #[test]
    fn fill_previous_works() {
        for (data, expected, from, to) in [
            (vec![], vec![], None, None),
            (vec![], vec![], Some(d("2100-01-01")), None),
            (
                vec![],
                vec![v("2100-01-01", "0")],
                Some(d("2100-01-01")),
                Some(d("2100-01-01")),
            ),
            (
                vec![v("2022-01-01", "01")],
                vec![v("2022-01-01", "01")],
                Some(d("2022-01-01")),
                Some(d("2022-01-01")),
            ),
            (
                vec![v("2022-01-01", "01"), v("2022-01-02", "02")],
                vec![v("2022-01-01", "01"), v("2022-01-02", "02")],
                Some(d("2022-01-01")),
                Some(d("2022-01-02")),
            ),
            (
                vec![v("2022-01-01", "01")],
                vec![v("2022-01-01", "01"), v("2022-01-02", "01")],
                Some(d("2022-01-01")),
                Some(d("2022-01-02")),
            ),
            (
                vec![v("2022-01-02", "02")],
                vec![v("2022-01-01", "0"), v("2022-01-02", "02")],
                Some(d("2022-01-01")),
                Some(d("2022-01-02")),
            ),
            (
                vec![
                    v("2022-08-20", "20"),
                    v("2022-08-22", "22"),
                    v("2022-08-23", "23"),
                    v("2022-08-25", "25"),
                ],
                vec![
                    v("2022-08-18", "0"),
                    v("2022-08-19", "0"),
                    v("2022-08-20", "20"),
                    v("2022-08-21", "20"),
                    v("2022-08-22", "22"),
                    v("2022-08-23", "23"),
                    v("2022-08-24", "23"),
                    v("2022-08-25", "25"),
                    v("2022-08-26", "25"),
                    v("2022-08-27", "25"),
                ],
                Some(d("2022-08-18")),
                Some(d("2022-08-27")),
            ),
            (
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-15", "12"),
                ],
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-11", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-13", "12"),
                    v("2023-07-14", "12"),
                    v("2023-07-15", "12"),
                ],
                Some(d("2023-07-12")),
                Some(d("2023-07-14")),
            ),
            (
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-15", "12"),
                ],
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-11", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-13", "12"),
                    v("2023-07-14", "12"),
                    v("2023-07-15", "12"),
                ],
                Some(d("2023-07-12")),
                Some(d("2023-07-13")),
            ),
        ] {
            let actual =
                fill_missing_points(data, MissingDatePolicy::FillPrevious, from, to, None).unwrap();
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn limits_are_respected() {
        let limit = Duration::days(4);
        assert_eq!(
            fill_missing_points(
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-15", "12"),
                ],
                MissingDatePolicy::FillZero,
                Some(d("2023-07-12")),
                Some(d("2023-07-12")),
                Some(limit)
            ),
            Err(ReadError::IntervalLimitExceeded(limit))
        );
        assert_eq!(
            fill_missing_points(
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-14", "12"),
                ],
                MissingDatePolicy::FillZero,
                Some(d("2023-07-10")),
                Some(d("2023-07-14")),
                Some(limit)
            ),
            Ok(vec![
                v("2023-07-10", "10"),
                v("2023-07-11", "0"),
                v("2023-07-12", "12"),
                v("2023-07-13", "0"),
                v("2023-07-14", "12"),
            ],)
        );
        assert_eq!(
            fill_missing_points(
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-14", "12"),
                ],
                MissingDatePolicy::FillZero,
                Some(d("2023-07-10")),
                Some(d("2023-07-15")),
                Some(limit)
            ),
            Err(ReadError::IntervalLimitExceeded(limit))
        );
        assert_eq!(
            fill_missing_points(
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-14", "12"),
                ],
                MissingDatePolicy::FillZero,
                Some(d("2023-07-09")),
                Some(d("2023-07-14")),
                Some(limit)
            ),
            Err(ReadError::IntervalLimitExceeded(limit))
        );
    }
}
