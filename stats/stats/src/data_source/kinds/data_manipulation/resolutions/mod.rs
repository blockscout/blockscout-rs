//! Construction of other resolutions from data sources of the same
//! type/meaning.
//! E.g. "weekly average block rewards" from "daily average block rewards".

use chrono::{DateTime, Utc};

use crate::{range::UniversalRange, types::Timespan};

pub mod average;
pub mod last_value;
pub mod sum;

// Boundaries of resulting range - timespans that contain boundaries of date range
fn date_range_to_timespan<T: Timespan>(range: UniversalRange<DateTime<Utc>>) -> UniversalRange<T> {
    let (start, end_inclusive) = range.into_inclusive_pair();
    let start_timespan = start.map(|s| T::from_date(s.date_naive()));
    let end_timespan = end_inclusive.map(|e| T::from_date(e.date_naive()));
    (start_timespan..=end_timespan).into()
}

pub fn extend_to_timespan_boundaries<T: Timespan + Ord>(
    range: UniversalRange<DateTime<Utc>>,
) -> UniversalRange<DateTime<Utc>> {
    let timespan_range = date_range_to_timespan::<T>(range);
    // start of timespan containing range start
    let (start, end) = timespan_range.into_inclusive_pair();
    let start = start.map(|s| s.saturating_start_timestamp());
    // start of timespan following range end (to get exclusive range again)
    let end = end.map(|e| {
        let timespan_after_range = e.saturating_next_timespan();
        timespan_after_range.saturating_start_timestamp()
    });
    (start..end).into()
}

/// Produce vector of timespan data `LResPoint` from vector of smaller timespan data `HResPoint`.
///
/// Combine all points that fall within one `LResPoint` timespan according to `reduce_timespan`.
///
/// `list` must be sorted (all equal timespans must be adjacent, as well as timespans
/// mapping into the same larger timespan); otherwise the correct result is not guaranteed.
pub fn reduce_each_timespan<HResPoint, LResPoint, LTimespan, R, M>(
    list: Vec<HResPoint>,
    timespan_mapping: M,
    reduce_timespan: R,
) -> Vec<LResPoint>
where
    M: Fn(&HResPoint) -> LTimespan,
    R: Fn(Vec<HResPoint>) -> LResPoint,
    LTimespan: Eq,
{
    let mut result = vec![];
    let mut current_l_points = vec![];
    let Some(mut current_l) = list.first().map(&timespan_mapping) else {
        return vec![];
    };
    for point in list {
        let this_l = timespan_mapping(&point);
        if this_l != current_l {
            current_l = this_l;
            let reduced = reduce_timespan(std::mem::take(&mut current_l_points));
            result.push(reduced);
        }
        current_l_points.push(point);
    }
    if !current_l_points.is_empty() {
        let reduced = reduce_timespan(std::mem::take(&mut current_l_points));
        result.push(reduced);
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::point_construction::{d, dt, week_of},
        types::timespans::Week,
    };

    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn date_range_to_timespan_weeks_works() {
        // weeks for this month (2024-07) are
        // 8-14, 15-21, 22-28

        assert_eq!(
            date_range_to_timespan::<Week>(
                (dt("2024-07-08T09:00:00").and_utc()..dt("2024-07-14T09:00:00").and_utc()).into()
            )
            .try_into_inclusive()
            .unwrap(),
            week_of("2024-07-08")..=week_of("2024-07-08")
        );
        assert_eq!(
            date_range_to_timespan::<Week>(
                (dt("2024-07-08T09:00:00").and_utc()..dt("2024-07-14T23:59:59").and_utc()).into()
            )
            .try_into_inclusive()
            .unwrap(),
            week_of("2024-07-08")..=week_of("2024-07-08")
        );
        assert_eq!(
            date_range_to_timespan::<Week>(
                (dt("2024-07-08T09:00:00").and_utc()..dt("2024-07-15T00:00:00").and_utc()).into()
            )
            .try_into_inclusive()
            .unwrap(),
            week_of("2024-07-08")..=week_of("2024-07-08")
        );
        assert_eq!(
            date_range_to_timespan::<Week>(
                (dt("1995-12-31T09:00:00").and_utc()..dt("1995-12-31T23:59:60").and_utc()).into()
            )
            .try_into_inclusive()
            .unwrap(),
            week_of("1995-12-31")..=week_of("1995-12-31")
        );
        assert_eq!(
            date_range_to_timespan::<Week>(
                (dt("1995-12-31T09:00:00").and_utc()..dt("1996-01-01T00:00:00").and_utc()).into()
            )
            .try_into_inclusive()
            .unwrap(),
            week_of("1995-12-31")..=week_of("1995-12-31")
        );

        assert_eq!(
            date_range_to_timespan::<Week>(
                (dt("2024-07-08T09:00:00").and_utc()..dt("2024-07-15T00:00:01").and_utc()).into()
            )
            .try_into_inclusive()
            .unwrap(),
            week_of("2024-07-08")..=week_of("2024-07-15")
        );
        assert_eq!(
            date_range_to_timespan::<Week>(
                (dt("1995-12-31T09:00:00").and_utc()..dt("1996-01-01T00:00:01").and_utc()).into()
            )
            .try_into_inclusive()
            .unwrap(),
            week_of("1995-12-31")..=week_of("1996-01-01")
        );
    }

    #[test]
    fn reduce_each_timespan_works() {
        // weeks for this month are
        // 8-14, 15-21, 22-28
        assert_eq!(
            reduce_each_timespan(
                vec![
                    d("2024-07-08"),
                    d("2024-07-09"),
                    d("2024-07-13"),
                    d("2024-07-15"),
                    d("2024-07-21"),
                ],
                |d| Week::from_date(*d),
                |a| a
            ),
            vec![
                vec![d("2024-07-08"), d("2024-07-09"), d("2024-07-13"),],
                vec![d("2024-07-15"), d("2024-07-21"),]
            ]
        )
    }
}
