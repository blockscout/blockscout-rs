use std::{cmp::Ordering, fmt::Debug};

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::{
    ResolutionKind, impl_into_string_timespan_value,
    types::{ConsistsOf, Timespan, TimespanDuration},
};

use super::Month;

/// Year number within range of `chrono::NaiveDate`
#[derive(Copy, Clone)]
pub struct Year(i32);

impl PartialEq for Year {
    fn eq(&self, other: &Self) -> bool {
        self.number_within_naive_date() == other.number_within_naive_date()
    }
}

impl Eq for Year {}

impl PartialOrd for Year {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Year {
    fn cmp(&self, other: &Self) -> Ordering {
        self.number_within_naive_date()
            .cmp(&other.number_within_naive_date())
    }
}

impl Debug for Year {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Year")
            .field(&self.number_within_naive_date())
            .finish()
    }
}

impl Year {
    /// First day of the year or `NaiveDate::MIN`
    pub fn saturating_first_day(&self) -> NaiveDate {
        NaiveDate::from_yo_opt(self.0, 1).unwrap_or(NaiveDate::MIN)
    }

    /// Number of the year (within `NaiveDate::MIN`)
    pub fn number_within_naive_date(self) -> i32 {
        self.clamp_by_naive_date_range().0
    }

    fn year_number_within_naive_date(year: i32) -> bool {
        NaiveDate::from_yo_opt(year, 1).is_some()
    }

    fn clamp_by_naive_date_range(self) -> Self {
        if Self::year_number_within_naive_date(self.0) {
            self
        } else if self.0 > 0 {
            <Self as Timespan>::max()
        } else {
            <Self as Timespan>::min()
        }
    }
}

impl Timespan for Year {
    fn from_date(date: NaiveDate) -> Self {
        Self(date.year())
    }

    fn into_date(self) -> NaiveDate {
        self.clamp_by_naive_date_range().saturating_first_day()
    }

    fn enum_variant() -> ResolutionKind {
        ResolutionKind::Year
    }

    fn saturating_start_timestamp(&self) -> DateTime<Utc> {
        self.saturating_first_day().saturating_start_timestamp()
    }

    fn saturating_add(&self, duration: TimespanDuration<Self>) -> Self
    where
        Self: Sized,
    {
        let add_years = duration.repeats().try_into().unwrap_or(i32::MAX);
        Self(self.number_within_naive_date().saturating_add(add_years)).clamp_by_naive_date_range()
    }

    fn saturating_sub(&self, duration: TimespanDuration<Self>) -> Self
    where
        Self: Sized,
    {
        let sub_years: i32 = duration.repeats().try_into().unwrap_or(i32::MAX);
        Self(self.number_within_naive_date().saturating_sub(sub_years)).clamp_by_naive_date_range()
    }

    fn checked_add(&self, duration: TimespanDuration<Self>) -> Option<Self>
    where
        Self: Sized,
    {
        let add_years = duration.repeats().try_into().ok()?;
        let new_year_num = self.number_within_naive_date().checked_add(add_years)?;
        if Self::year_number_within_naive_date(new_year_num) {
            Some(Self(new_year_num))
        } else {
            None
        }
    }

    fn checked_sub(&self, duration: TimespanDuration<Self>) -> Option<Self>
    where
        Self: Sized,
    {
        let sub_years: i32 = duration.repeats().try_into().ok()?;
        let new_year_num = self.number_within_naive_date().checked_sub(sub_years)?;
        if Self::year_number_within_naive_date(new_year_num) {
            Some(Self(new_year_num))
        } else {
            None
        }
    }

    fn max() -> Self {
        Self::from_date(NaiveDate::MAX)
    }

    fn min() -> Self {
        Self::from_date(NaiveDate::MIN)
    }
}

impl ConsistsOf<NaiveDate> for Year {
    fn from_smaller(date: NaiveDate) -> Self {
        Year::from_date(date)
    }

    fn into_smaller(self) -> NaiveDate {
        Year::into_date(self)
    }
}

impl ConsistsOf<Month> for Year {
    fn from_smaller(month: Month) -> Self {
        Year::from_date(month.into_date())
    }

    fn into_smaller(self) -> Month {
        Month::from_date(Year::into_date(self))
    }
}

impl_into_string_timespan_value!(Year, i64);
impl_into_string_timespan_value!(Year, f64);
impl_into_string_timespan_value!(Year, Decimal);

#[cfg(test)]
mod tests {
    use crate::{
        tests::point_construction::{d, dt, month_of},
        utils::day_start,
    };

    use super::*;

    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn year_date_conversion_works() {
        assert_eq!(Year::from_date(d("2015-01-01")), Year(2015));
        assert_eq!(Year::from_date(d("2015-12-31")), Year(2015));
        assert_eq!(Year::from_date(d("2012-02-29")), Year(2012));
        assert_eq!(Year::from_date(NaiveDate::MAX), Year(NaiveDate::MAX.year()));
        assert_eq!(Year::from_date(NaiveDate::MIN), Year(NaiveDate::MIN.year()));
        assert_eq!(Year(2015).into_date(), d("2015-01-01"));
        assert_eq!(Year(2012).into_date(), d("2012-01-01"));
        assert_eq!(Year(i32::MIN).into_date(), NaiveDate::MIN);
        assert_eq!(
            Year(i32::MAX).into_date(),
            NaiveDate::from_yo_opt(NaiveDate::MAX.year(), 1).unwrap()
        );
    }

    #[test]
    fn year_month_conversion_works() {
        assert_eq!(Year::from_smaller(month_of("2015-01-01")), Year(2015));
        assert_eq!(Year::from_smaller(month_of("2015-12-31")), Year(2015));
        assert_eq!(Year::from_smaller(month_of("2012-02-29")), Year(2012));
        assert_eq!(
            Year::from_smaller(Month::from_date(NaiveDate::MAX)),
            Year(NaiveDate::MAX.year())
        );
        assert_eq!(
            Year::from_smaller(Month::from_date(NaiveDate::MIN)),
            Year(NaiveDate::MIN.year())
        );
        assert_eq!(
            <Year as ConsistsOf<Month>>::into_smaller(Year(2015)),
            month_of("2015-01-01")
        );
        assert_eq!(
            <Year as ConsistsOf<Month>>::into_smaller(Year(2012)),
            month_of("2012-01-01")
        );
        assert_eq!(
            <Year as ConsistsOf<Month>>::into_smaller(Year(i32::MIN)),
            Month::from_date(NaiveDate::MIN)
        );
        assert_eq!(
            <Year as ConsistsOf<Month>>::into_smaller(Year(i32::MAX)),
            Month::from_date(NaiveDate::from_yo_opt(NaiveDate::MAX.year(), 1).unwrap())
        );
    }

    #[test]
    fn year_eq_works() {
        // all years with inner values outside `NaiveDate` range are treated equally
        // (in order not to panic on such cases).
        // this test checks that eq works for this case
        let max_date_year = Year::from_date(NaiveDate::MAX);
        assert_eq!(max_date_year, Year(max_date_year.0 + 1));
        assert_eq!(max_date_year, Year(i32::MAX));
        assert_ne!(max_date_year, Year(max_date_year.0 - 1));

        let min_date_year = Year::from_date(NaiveDate::MIN);
        assert_eq!(min_date_year, Year(min_date_year.0 - 1));
        assert_eq!(min_date_year, Year(i32::MIN));
        assert_ne!(min_date_year, Year(min_date_year.0 + 1));
    }

    #[test]
    fn year_saturating_first_day_works() {
        assert_eq!(
            Year::from_date(d("2015-01-01")).saturating_first_day(),
            d("2015-01-01")
        );
        assert_eq!(
            Year::from_date(d("2015-01-02")).saturating_first_day(),
            d("2015-01-01")
        );
        assert_eq!(
            Year::from_date(d("2015-12-31")).saturating_first_day(),
            d("2015-01-01")
        );
        assert_eq!(
            Year::from_date(NaiveDate::MAX).saturating_first_day(),
            NaiveDate::from_yo_opt(NaiveDate::MAX.year(), 1).unwrap()
        );
        // saturation works
        assert_eq!(
            Year::from_date(NaiveDate::MIN).saturating_first_day(),
            NaiveDate::MIN
        );
    }

    #[test]
    fn year_saturating_arithmetics_works() {
        assert_eq!(
            Year(2016).saturating_add(TimespanDuration::from_timespan_repeats(16)),
            Year(2032)
        );
        assert_eq!(
            Year(2016).saturating_sub(TimespanDuration::from_timespan_repeats(16)),
            Year(2000)
        );

        assert_eq!(
            Year(2016).saturating_add(TimespanDuration::from_timespan_repeats(u64::MAX)),
            Year::from_date(NaiveDate::MAX)
        );
        assert_eq!(
            Year(2016).saturating_sub(TimespanDuration::from_timespan_repeats(u64::MAX)),
            Year::from_date(NaiveDate::MIN)
        );

        assert_eq!(
            Year::from_date(NaiveDate::MAX)
                .saturating_add(TimespanDuration::from_timespan_repeats(1)),
            Year::from_date(NaiveDate::MAX)
        );
        assert_eq!(
            Year::from_date(NaiveDate::MIN)
                .saturating_sub(TimespanDuration::from_timespan_repeats(1)),
            Year::from_date(NaiveDate::MIN)
        );
    }
    #[test]
    fn year_checked_add_works() {
        assert_eq!(
            Year(2016).checked_add(TimespanDuration::from_timespan_repeats(16)),
            Some(Year(2032))
        );
        assert_eq!(
            Year(2016).checked_add(TimespanDuration::from_timespan_repeats(0)),
            Some(Year(2016))
        );
        assert_eq!(
            Year(2016).checked_add(TimespanDuration::from_timespan_repeats(u64::MAX)),
            None
        );
        assert_eq!(
            Year::from_date(NaiveDate::MAX).checked_add(TimespanDuration::from_timespan_repeats(1)),
            None
        );
        assert_eq!(
            Year(i32::MAX - 5).checked_add(TimespanDuration::from_timespan_repeats(3)),
            None
        );
    }

    #[test]
    fn year_checked_sub_works() {
        assert_eq!(
            Year(2016).checked_sub(TimespanDuration::from_timespan_repeats(16)),
            Some(Year(2000))
        );
        assert_eq!(
            Year(2016).checked_sub(TimespanDuration::from_timespan_repeats(0)),
            Some(Year(2016))
        );
        assert_eq!(
            Year(2016).checked_sub(TimespanDuration::from_timespan_repeats(u64::MAX)),
            None
        );
        assert_eq!(
            Year::from_date(NaiveDate::MIN).checked_sub(TimespanDuration::from_timespan_repeats(1)),
            None
        );
        assert_eq!(
            Year(i32::MIN + 5).checked_sub(TimespanDuration::from_timespan_repeats(3)),
            None
        );
    }

    #[test]
    fn year_saturating_first_timestamp_works() {
        assert_eq!(
            Year::from_date(d("2015-01-01")).saturating_start_timestamp(),
            dt("2015-01-01T00:00:00").and_utc()
        );
        assert_eq!(
            Year::from_date(d("2015-12-31")).saturating_start_timestamp(),
            dt("2015-01-01T00:00:00").and_utc()
        );
        assert_eq!(
            Year::from_date(NaiveDate::MAX).saturating_start_timestamp(),
            day_start(&NaiveDate::from_yo_opt(NaiveDate::MAX.year(), 1).unwrap())
        );
        assert_eq!(
            Year::from_date(NaiveDate::MIN).saturating_start_timestamp(),
            DateTime::<Utc>::MIN_UTC
        );
    }
}
