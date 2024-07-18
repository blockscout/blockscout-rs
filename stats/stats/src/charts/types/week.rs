use std::ops::{Range, RangeInclusive};

use chrono::{Days, NaiveDate, NaiveWeek, Weekday};
use rust_decimal::Decimal;

use crate::charts::chart_properties_portrait::imports::ResolutionKind;

use super::TimespanValue;

pub const WEEK_START: Weekday = Weekday::Mon;

pub struct Week(NaiveWeek);

impl std::fmt::Debug for Week {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut b = f.debug_tuple("Week");
        if let Some(days) = self.checked_days() {
            b.field(&days.start());
            b.field(&days.end());
        } else if let Some(first) = self.checked_first_day() {
            b.field(&first);
            b.field(&"overflow");
        } else if let Some(last) = self.checked_last_day() {
            b.field(&"overflow");
            b.field(&last);
        } else {
            // at least one of `checked_first_day` or `checked_last_day`
            // must be some
            b.field(&"invalid");
        }
        b.finish()
    }
}

// `NaiveWeek` doesn't implement common traits, we have to do it by hand

impl PartialEq for Week {
    fn eq(&self, other: &Self) -> bool {
        // if both `None` then it's `NaiveDate::MIN`'s week,
        // thus equal
        self.checked_first_day().eq(&other.checked_first_day())
    }
}

impl Eq for Week {}

impl PartialOrd for Week {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // `None` means `NaiveDate::MIN`'s week
        // which fits well with `Option`'s "`None` is less than `Some`"
        // policy
        self.checked_first_day()
            .partial_cmp(&other.checked_first_day())
    }
}

impl Ord for Week {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // `None` means `NaiveDate::MIN`'s week
        // which fits well with `Option`'s "`None` is less than `Some`"
        // policy
        self.checked_first_day().cmp(&other.checked_first_day())
    }
}

impl Clone for Week {
    fn clone(&self) -> Self {
        // Saturation happens only in one week
        Self::new(self.saturating_first_day())
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
        self.saturating_first_day()
    }

    fn next_timespan(&self) -> Self {
        self.saturating_next_week()
    }

    fn previous_timespan(&self) -> Self {
        self.saturating_previous_week()
    }

    fn enum_variant() -> ResolutionKind {
        ResolutionKind::Week
    }

    fn start_timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        self.saturating_first_day().start_timestamp()
    }

    fn add_duration(&self, duration: super::TimespanDuration<Self>) -> Self
    where
        Self: Sized,
    {
        let result_week_date = self
            .saturating_first_day()
            .checked_add_days(Days::new(duration.repeats() * 7))
            .unwrap_or(NaiveDate::MAX);
        Self::from_date(result_week_date)
    }

    fn sub_duration(&self, duration: super::TimespanDuration<Self>) -> Self
    where
        Self: Sized,
    {
        let result_week_date = self
            .saturating_first_day()
            .checked_sub_days(Days::new(duration.repeats() * 7))
            .unwrap_or(NaiveDate::MIN);
        Self::from_date(result_week_date)
    }
}

impl Week {
    pub fn new(day: NaiveDate) -> Week {
        Self(day.week(WEEK_START))
    }

    /// `None` - out of range
    pub fn checked_days(&self) -> Option<RangeInclusive<NaiveDate>> {
        Some(self.checked_first_day()?..=self.checked_last_day()?)
    }

    /// `None` - out of range (week of `NaiveDate::MIN`)
    pub fn checked_first_day(&self) -> Option<NaiveDate> {
        // Current interface of `NaiveWeek` does not provide any way
        // of idiomatic overflow handling
        std::panic::catch_unwind(|| self.0.first_day()).ok()
    }

    /// First day of the week or `NaiveDate::MIN`
    pub fn saturating_first_day(&self) -> NaiveDate {
        self.checked_first_day().unwrap_or(NaiveDate::MIN)
    }

    /// `None` - out of range (week of `NaiveDate::MAX`)
    pub fn checked_last_day(&self) -> Option<NaiveDate> {
        // Current interface of `NaiveWeek` does not provide any way
        // of idiomatic overflow handling
        std::panic::catch_unwind(|| self.0.last_day()).ok()
    }

    /// Last day of the week or `NaiveDate::MAX`
    pub fn saturating_last_day(&self) -> NaiveDate {
        self.checked_last_day().unwrap_or(NaiveDate::MAX)
    }

    pub fn saturating_next_week(&self) -> Week {
        let next_week_first_day = self
            .saturating_last_day()
            .checked_add_days(Days::new(1))
            .unwrap_or(NaiveDate::MAX);
        Week::new(next_week_first_day)
    }

    pub fn saturating_previous_week(&self) -> Week {
        let prev_week_last_day = self
            .saturating_first_day()
            .checked_sub_days(Days::new(1))
            .unwrap_or(NaiveDate::MIN);
        Week::new(prev_week_last_day)
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
        impl From<$impl_for> for WeekValue<String> {
            fn from(value: $impl_for) -> Self {
                Self {
                    timespan: value.timespan,
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

    #[test]
    fn next_and_prev_saturates() {
        let week_max = Week::new(NaiveDate::MAX);
        assert_eq!(week_max, week_max.saturating_next_week());
        let week_min = Week::new(NaiveDate::MIN);
        assert_eq!(week_min, week_min.saturating_previous_week());
    }
}
