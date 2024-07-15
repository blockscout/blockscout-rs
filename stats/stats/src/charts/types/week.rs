use std::ops::{Range, RangeInclusive};

use chrono::{Days, NaiveDate, NaiveWeek, Weekday};
use rust_decimal::Decimal;

use super::{db::DbDateValue, TimespanValue};

pub const WEEK_START: Weekday = Weekday::Mon;

pub struct Week(NaiveWeek);

impl std::fmt::Debug for Week {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Week").field(&self.days()).finish()
    }
}

// `NaiveWeek` doesn't implement common traits, we have to do it by hand

impl PartialEq for Week {
    fn eq(&self, other: &Self) -> bool {
        self.0.first_day().eq(&other.0.first_day())
    }
}

impl Eq for Week {}

impl PartialOrd for Week {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.first_day().partial_cmp(&other.0.first_day())
    }
}

impl Ord for Week {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.first_day().cmp(&other.0.first_day())
    }
}

impl Clone for Week {
    fn clone(&self) -> Self {
        Self::new(self.0.first_day())
    }
}

// todo: check if need removal

// impl TryGetable for Week {
//     fn try_get_by<I: sea_orm::ColIdx>(
//         res: &sea_orm::QueryResult,
//         index: I,
//     ) -> Result<Self, sea_orm::TryGetError> {
//         NaiveDate::try_get_by(res, index).map(|d| Week::new(d))
//     }
// }

impl super::Timespan for Week {
    fn from_date(date: NaiveDate) -> Self {
        Week::new(date)
    }

    fn into_date(self) -> NaiveDate {
        self.0.days().start().clone()
    }

    fn next_timespan(self) -> Self {
        self.saturating_next_week()
    }
}

impl Week {
    pub fn new(day: NaiveDate) -> Week {
        Self(day.week(WEEK_START))
    }

    pub fn days(&self) -> RangeInclusive<NaiveDate> {
        self.0.days()
    }

    pub fn saturating_next_week(&self) -> Week {
        let next_week_first_day = self
            .0
            .last_day()
            .checked_add_days(Days::new(1))
            .unwrap_or(NaiveDate::MAX);
        Week::new(next_week_first_day)
    }

    // iterating over ranges with custom inner types is nightly-only
    // https://users.rust-lang.org/t/implement-trait-step-in-1-76-0/108204/8
    pub fn range_into_iter(range: Range<Week>) -> WeekRangeIterator {
        WeekRangeIterator(range)
    }

    // iterating over ranges with custom inner types is nightly-only
    // https://users.rust-lang.org/t/implement-trait-step-in-1-76-0/108204/8
    pub fn range_inclusive_into_iter(range: RangeInclusive<Week>) -> WeekRangeIterator {
        let (start, end) = range.into_inner();
        WeekRangeIterator(start..(&end).saturating_next_week())
    }
}

pub struct WeekRangeIterator(Range<Week>);

impl Iterator for WeekRangeIterator {
    type Item = Week;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            None
        } else {
            let next = self.0.start.clone();
            self.0.start = next.saturating_next_week();
            Some(next)
        }
    }
}

pub type WeekValue<V> = TimespanValue<Week, V>;

// todo: remove??

/// Implement non-string date-value type
macro_rules! impl_into_string_date_value {
    ($impl_for:ty) => {
        impl From<$impl_for> for DbDateValue<String> {
            fn from(value: $impl_for) -> Self {
                Self {
                    date: *value.timespan.days().start(),
                    value: value.value.to_string(),
                }
            }
        }
    };
}

impl_into_string_date_value!(WeekValue<i64>);
impl_into_string_date_value!(WeekValue<f64>);
impl_into_string_date_value!(WeekValue<Decimal>);

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::tests::point_construction::week_of;

    use super::*;

    #[test]
    fn week_eq_works() {
        // weeks for this month are
        // 8-14, 15-21

        assert_eq!(week_of("2024-07-08"), week_of("2024-07-08"));
        assert_eq!(week_of("2024-07-08"), week_of("2024-07-09"));
        assert_eq!(week_of("2024-07-08"), week_of("2024-07-14"));

        assert_eq!(week_of("2024-07-15"), week_of("2024-07-15"));
        assert_eq!(week_of("2024-07-15"), week_of("2024-07-21"));

        assert_ne!(week_of("2024-07-14"), week_of("2024-07-15"));
    }

    #[test]
    fn week_iterator_works() {
        // weeks for this month are
        // 8-14, 15-21, 22-28

        for range in [
            week_of("2024-07-08")..week_of("2024-07-22"),
            week_of("2024-07-08")..week_of("2024-07-28"),
            week_of("2024-07-14")..week_of("2024-07-28"),
        ] {
            assert_eq!(
                Week::range_into_iter(range.clone()).collect_vec(),
                vec![week_of("2024-07-08"), week_of("2024-07-15")]
            );
            assert_eq!(
                Week::range_inclusive_into_iter(range.start..=range.end).collect_vec(),
                vec![
                    week_of("2024-07-08"),
                    week_of("2024-07-15"),
                    week_of("2024-07-22")
                ]
            );
        }

        for range in [
            week_of("2024-07-08")..week_of("2024-07-15"),
            week_of("2024-07-08")..week_of("2024-07-21"),
            week_of("2024-07-14")..week_of("2024-07-21"),
        ] {
            assert_eq!(
                Week::range_into_iter(range.clone()).collect_vec(),
                vec![week_of("2024-07-08")]
            );
            assert_eq!(
                Week::range_inclusive_into_iter(range.start..=range.end).collect_vec(),
                vec![week_of("2024-07-08"), week_of("2024-07-15")]
            );
        }
    }
}
