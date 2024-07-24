//! Construction of other resolutions from data sources of the same
//! type/meaning.
//! E.g. "weekly average block rewards" from "daily average block rewards".

use std::ops::{Range, RangeInclusive};

use chrono::{DateTime, Utc};

use crate::{types::Timespan, utils::exclusive_datetime_range_to_inclusive};

pub mod average;
// pub mod last_value;

// Boundaries of resulting range - timespans that contain boundaries of date range
fn date_range_to_timespan<T: Timespan>(range: Range<DateTime<Utc>>) -> RangeInclusive<T> {
    let range = exclusive_datetime_range_to_inclusive(range);
    let start_timespan = T::from_date(range.start().date_naive());
    let end_timespan = T::from_date(range.end().date_naive());
    start_timespan..=end_timespan
}

pub fn extend_to_timespan_boundaries<T: Timespan>(
    range: Range<DateTime<Utc>>,
) -> Range<DateTime<Utc>> {
    let timespan_range = date_range_to_timespan::<T>(range);
    // start of timespan containing range start
    let start: DateTime<Utc> = timespan_range.start().start_timestamp();
    // start of timespan following range end (to get exclusive range again)
    let timespan_after_range = timespan_range.end().saturating_next_timespan();
    let end = timespan_after_range.start_timestamp();
    start..end
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::point_construction::{dt, week_of},
        types::week::Week,
    };

    use super::*;

    use pretty_assertions::assert_eq;

    // todo: other timespans
    #[test]
    fn date_range_to_timespan_weeks_works() {
        // weeks for this month are
        // 8-14, 15-21, 22-28

        assert_eq!(
            date_range_to_timespan::<Week>(
                dt("2024-07-08T09:00:00").and_utc()..dt("2024-07-14T09:00:00").and_utc()
            ),
            week_of("2024-07-08")..=week_of("2024-07-08")
        );
        assert_eq!(
            date_range_to_timespan::<Week>(
                dt("2024-07-08T09:00:00").and_utc()..dt("2024-07-14T23:59:59").and_utc()
            ),
            week_of("2024-07-08")..=week_of("2024-07-08")
        );
        assert_eq!(
            date_range_to_timespan::<Week>(
                dt("2024-07-08T09:00:00").and_utc()..dt("2024-07-15T00:00:00").and_utc()
            ),
            week_of("2024-07-08")..=week_of("2024-07-08")
        );
        assert_eq!(
            date_range_to_timespan::<Week>(
                dt("1995-12-31T09:00:00").and_utc()..dt("1995-12-31T23:59:60").and_utc()
            ),
            week_of("1995-12-31")..=week_of("1995-12-31")
        );
        assert_eq!(
            date_range_to_timespan::<Week>(
                dt("1995-12-31T09:00:00").and_utc()..dt("1996-01-01T00:00:00").and_utc()
            ),
            week_of("1995-12-31")..=week_of("1995-12-31")
        );

        assert_eq!(
            date_range_to_timespan::<Week>(
                dt("2024-07-08T09:00:00").and_utc()..dt("2024-07-15T00:00:01").and_utc()
            ),
            week_of("2024-07-08")..=week_of("2024-07-15")
        );
        assert_eq!(
            date_range_to_timespan::<Week>(
                dt("1995-12-31T09:00:00").and_utc()..dt("1996-01-01T00:00:01").and_utc()
            ),
            week_of("1995-12-31")..=week_of("1996-01-01")
        );
    }
}
