use std::{cmp::Ordering, ops::RangeInclusive};

use chrono::{Days, NaiveDate, NaiveWeek, Weekday};
use rust_decimal::Decimal;

use crate::{
    charts::ResolutionKind,
    impl_into_string_timespan_value,
    types::{ConsistsOf, Timespan, TimespanDuration, TimespanValue},
};

pub const WEEK_START: Weekday = Weekday::Mon;

/// Week starting from Monday.
/// Mostly a wrapper around `NaiveWeek` but with additional useful
/// functionality
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
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Week {
    fn cmp(&self, other: &Self) -> Ordering {
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

impl Timespan for Week {
    fn from_date(date: NaiveDate) -> Self {
        Week::new(date)
    }

    fn into_date(self) -> NaiveDate {
        self.saturating_first_day()
    }

    fn enum_variant() -> ResolutionKind {
        ResolutionKind::Week
    }

    fn saturating_start_timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        self.saturating_first_day().saturating_start_timestamp()
    }

    fn saturating_add(&self, duration: TimespanDuration<Self>) -> Self
    where
        Self: Sized,
    {
        let result_week_date = self
            .saturating_first_day()
            .checked_add_days(Days::new(duration.repeats() * 7))
            .unwrap_or(NaiveDate::MAX);
        Self::from_date(result_week_date)
    }

    fn saturating_sub(&self, duration: TimespanDuration<Self>) -> Self
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

impl ConsistsOf<NaiveDate> for Week {
    fn from_smaller(date: NaiveDate) -> Self {
        Week::from_date(date)
    }

    fn into_smaller(self) -> NaiveDate {
        Week::into_date(self)
    }
}

impl Week {
    pub fn new(day: NaiveDate) -> Week {
        Self(day.week(WEEK_START))
    }

    /// `None` - out of range
    pub fn checked_days(&self) -> Option<RangeInclusive<NaiveDate>> {
        self.0.checked_days()
    }

    /// `None` - out of range (week of `NaiveDate::MIN`)
    pub fn checked_first_day(&self) -> Option<NaiveDate> {
        self.0.checked_first_day()
    }

    /// First day of the week or `NaiveDate::MIN`
    pub fn saturating_first_day(&self) -> NaiveDate {
        self.checked_first_day().unwrap_or(NaiveDate::MIN)
    }

    /// `None` - out of range (week of `NaiveDate::MAX`)
    pub fn checked_last_day(&self) -> Option<NaiveDate> {
        // Current interface of `NaiveWeek` does not provide any way
        // of idiomatic overflow handling
        self.0.checked_last_day()
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
}

pub type WeekValue<V> = TimespanValue<Week, V>;

impl_into_string_timespan_value!(Week, i64);
impl_into_string_timespan_value!(Week, f64);
impl_into_string_timespan_value!(Week, Decimal);

#[cfg(test)]
mod tests {
    use crate::tests::point_construction::{d, week_of};

    use super::*;

    #[test]
    fn week_into_date_works() {
        // expected to not change as this is used for storage
        assert_eq!(week_of("2024-07-08").into_date(), d("2024-07-08"));
        assert_eq!(week_of("2024-07-10").into_date(), d("2024-07-08"));
        assert_eq!(week_of("2024-07-14").into_date(), d("2024-07-08"));
        assert_eq!(Week::new(NaiveDate::MIN).into_date(), NaiveDate::MIN);
        assert_eq!(
            Week::new(NaiveDate::MAX).into_date(),
            NaiveDate::MAX.week(Weekday::Mon).first_day()
        );
    }

    // also indirectly tests conversion from date
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
    fn week_checked_days_works() {
        // weeks for this month are
        // 8-14, 15-21, 22-28
        assert_eq!(
            week_of("2024-07-08").checked_days(),
            Some(d("2024-07-08")..=d("2024-07-14"))
        );
        assert_eq!(
            week_of("2024-07-14").checked_days(),
            Some(d("2024-07-08")..=d("2024-07-14"))
        );
        assert_eq!(
            week_of("2024-07-15").checked_days(),
            Some(d("2024-07-15")..=d("2024-07-21"))
        );
        // not always must be true, but current `MAX` and `MIN`
        // are not Sunday & Monday respectively
        assert_eq!(Week::new(NaiveDate::MAX).checked_days(), None);
        assert_eq!(Week::new(NaiveDate::MIN).checked_days(), None);

        assert!(Week::new(NaiveDate::MAX)
            .saturating_previous_week()
            .checked_days()
            .is_some());
        assert!(Week::new(NaiveDate::MIN)
            .saturating_next_week()
            .checked_days()
            .is_some());
    }

    #[test]
    fn next_and_prev_saturates() {
        let week_max = Week::new(NaiveDate::MAX);
        assert_eq!(week_max, week_max.saturating_next_week());
        let week_min = Week::new(NaiveDate::MIN);
        assert_eq!(week_min, week_min.saturating_previous_week());
    }
}
