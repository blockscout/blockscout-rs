//! Tools for operating with missing data
use std::ops::RangeInclusive;

use crate::{
    charts::db_interaction::types::DateValue, DateValueString, MissingDatePolicy, ReadError,
};
use chrono::{Days, Duration, NaiveDate};

/// Fits the `data` within the range (`from`, `to`), preserving
/// information nearby the boundaries according to `policy`.
///
/// Assumes the data is sorted.
pub fn fit_into_range(
    mut data: Vec<DateValueString>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    policy: MissingDatePolicy,
) -> Vec<DateValueString> {
    let trim_range =
        RangeInclusive::new(from.unwrap_or(NaiveDate::MIN), to.unwrap_or(NaiveDate::MAX));
    match policy {
        MissingDatePolicy::FillZero => {
            // (potential) missing values at the boundaries
            // will be just considered zero
            trim_out_of_range_sorted(&mut data, trim_range);
            data
        }
        MissingDatePolicy::FillPrevious => {
            // preserve the point before the range (if needed)
            if let Some(from) = from {
                if let Some(last_point_before) =
                    data.iter().take_while(|p| p.get_parts().0 < &from).last()
                {
                    if let Err(insert_idx) =
                        data.binary_search_by_key(&from, |p| *p.get_parts().0)
                    {
                        // `data` does not contain point for `from`, need to insert by `FillPrevious` logic
                        let new_point =
                            DateValueString::from_parts(from, last_point_before.value.clone());
                        data.insert(insert_idx, new_point);
                    }
                }
            }
            trim_out_of_range_sorted(&mut data, trim_range);
            data
        }
    }
}

/// the vector must be sorted
pub fn trim_out_of_range_sorted<DV: DateValue>(
    data: &mut Vec<DV>,
    range: RangeInclusive<NaiveDate>,
) {
    // start of relevant section
    let keep_from_idx = data
        .binary_search_by_key(&range.start(), |p| p.get_parts().0)
        .unwrap_or_else(|i| i);
    // irrelevant tail start
    let trim_from_idx = data
        .binary_search_by_key(
            &(range
                .end()
                .checked_add_days(Days::new(1))
                .unwrap_or(NaiveDate::MAX)),
            |p| *p.get_parts().0,
        )
        .unwrap_or_else(|i| i);
    data.truncate(trim_from_idx);
    data.drain(..keep_from_idx);
}

/// Fills missing points according to policy and filters out points outside of range.
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
    use crate::{
        charts::db_interaction::types::DateValueInt,
        tests::point_construction::{d, v, v_int},
    };

    use super::*;

    use pretty_assertions::assert_eq;

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

    #[test]
    fn trim_range_works() {
        // Empty vector
        let mut data: Vec<DateValueInt> = vec![];
        trim_out_of_range_sorted(&mut data, d("2100-01-02")..=d("2100-01-04"));
        assert_eq!(data, vec![]);
        trim_out_of_range_sorted(&mut data, NaiveDate::MIN..=NaiveDate::MAX);

        // No elements in range (before)
        let mut data = vec![
            v_int("2100-01-01", 1),
            v_int("2100-01-02", 2),
            v_int("2100-01-03", 3),
        ];
        trim_out_of_range_sorted(&mut data, d("2099-12-30")..=d("2099-12-31"));
        assert_eq!(data, vec![]);

        // No elements in range (after)
        let mut data = vec![
            v_int("2100-01-01", 1),
            v_int("2100-01-02", 2),
            v_int("2100-01-03", 3),
        ];
        trim_out_of_range_sorted(&mut data, d("2100-01-04")..=d("2100-01-05"));
        assert_eq!(data, vec![]);

        // All elements in range
        let mut data = vec![
            v_int("2100-01-01", 1),
            v_int("2100-01-02", 2),
            v_int("2100-01-03", 3),
        ];
        trim_out_of_range_sorted(&mut data, d("2100-01-01")..=d("2100-01-03"));
        assert_eq!(
            data,
            vec![
                v_int("2100-01-01", 1),
                v_int("2100-01-02", 2),
                v_int("2100-01-03", 3),
            ]
        );

        // Partial elements in range
        let mut data = vec![
            v_int("2100-01-01", 1),
            v_int("2100-01-02", 2),
            v_int("2100-01-03", 3),
        ];
        trim_out_of_range_sorted(&mut data, d("2100-01-02")..=d("2100-01-10"));
        assert_eq!(data, vec![v_int("2100-01-02", 2), v_int("2100-01-03", 3)]);

        // Single element in range
        let mut data = vec![
            v_int("2100-01-01", 1),
            v_int("2100-01-02", 2),
            v_int("2100-01-03", 3),
        ];
        trim_out_of_range_sorted(&mut data, d("2100-01-02")..=d("2100-01-02"));
        assert_eq!(data, vec![v_int("2100-01-02", 2)]);
    }

    #[test]
    fn fit_into_range_works() {
        // Empty vector
        assert_eq!(
            fit_into_range(
                vec![],
                Some(d("2100-01-02")),
                Some(d("2100-01-04")),
                MissingDatePolicy::FillZero
            ),
            vec![]
        );
        assert_eq!(
            fit_into_range(
                vec![],
                Some(d("2100-01-02")),
                Some(d("2100-01-04")),
                MissingDatePolicy::FillPrevious
            ),
            vec![]
        );

        let data = vec![
            v("2100-01-01", "1"),
            v("2100-01-02", "2"),
            v("2100-01-03", "3"),
        ];

        // No elements in range
        assert_eq!(
            fit_into_range(
                data.clone(),
                Some(d("2099-12-30")),
                Some(d("2099-12-31")),
                MissingDatePolicy::FillZero
            ),
            vec![]
        );
        assert_eq!(
            fit_into_range(
                data.clone(),
                Some(d("2099-12-04")),
                Some(d("2099-12-05")),
                MissingDatePolicy::FillZero
            ),
            vec![]
        );
        assert_eq!(
            fit_into_range(
                data.clone(),
                Some(d("2099-12-30")),
                Some(d("2099-12-31")),
                MissingDatePolicy::FillPrevious
            ),
            vec![]
        );
        assert_eq!(
            fit_into_range(
                data.clone(),
                Some(d("2099-12-04")),
                Some(d("2099-12-05")),
                MissingDatePolicy::FillPrevious
            ),
            vec![]
        );
        // All elements in range
        assert_eq!(
            fit_into_range(
                data.clone(),
                Some(d("2100-01-01")),
                Some(d("2100-01-03")),
                MissingDatePolicy::FillZero
            ),
            vec![
                v("2100-01-01", "1"),
                v("2100-01-02", "2"),
                v("2100-01-03", "3"),
            ]
        );

        // All elements in range with FillPrevious policy
        assert_eq!(
            fit_into_range(
                data.clone(),
                Some(d("2100-01-01")),
                Some(d("2100-01-03")),
                MissingDatePolicy::FillPrevious
            ),
            vec![
                v("2100-01-01", "1"),
                v("2100-01-02", "2"),
                v("2100-01-03", "3"),
            ]
        );

        // Partial elements in range
        let data = vec![
            v("2100-01-01", "1"),
            v("2100-01-02", "2"),
            v("2100-01-03", "3"),
        ];
        assert_eq!(
            fit_into_range(
                data.clone(),
                Some(d("2100-01-02")),
                Some(d("2100-01-10")),
                MissingDatePolicy::FillZero
            ),
            vec![v("2100-01-02", "2"), v("2100-01-03", "3"),]
        );
        assert_eq!(
            fit_into_range(
                data.clone(),
                Some(d("2100-01-02")),
                Some(d("2100-01-10")),
                MissingDatePolicy::FillPrevious
            ),
            vec![v("2100-01-02", "2"), v("2100-01-03", "3"),]
        );

        // Range includes dates outside of the provided data with FillZero policy
        let data = vec![
            v("2100-01-03", "3"),
            v("2100-01-05", "5"),
            v("2100-01-07", "7"),
        ];
        assert_eq!(
            fit_into_range(
                data.clone(),
                Some(d("2100-01-04")),
                Some(d("2100-01-06")),
                MissingDatePolicy::FillZero
            ),
            vec![v("2100-01-05", "5"),]
        );

        // Range includes dates outside of the provided data with FillPrevious policy
        let data = vec![
            v("2100-01-03", "3"),
            v("2100-01-05", "5"),
            v("2100-01-07", "7"),
        ];
        assert_eq!(
            fit_into_range(
                data.clone(),
                Some(d("2100-01-04")),
                Some(d("2100-01-06")),
                MissingDatePolicy::FillPrevious
            ),
            vec![v("2100-01-04", "3"), v("2100-01-05", "5"),]
        );
    }
}
