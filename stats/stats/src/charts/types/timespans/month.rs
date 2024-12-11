use std::cmp::Ordering;

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::{
    impl_into_string_timespan_value,
    types::{ConsistsOf, Timespan, TimespanDuration},
    ResolutionKind,
};

#[derive(Copy, Clone)]
pub struct Month {
    date_in_month: NaiveDate,
}

impl std::fmt::Debug for Month {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Month")
            .field("year", &self.date_in_month.year())
            .field("month", &self.date_in_month.month())
            .finish()
    }
}

impl PartialEq for Month {
    fn eq(&self, other: &Self) -> bool {
        self.saturating_first_day() == other.saturating_first_day()
    }
}

impl Eq for Month {}

impl PartialOrd for Month {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Month {
    fn cmp(&self, other: &Self) -> Ordering {
        self.saturating_first_day()
            .cmp(&other.saturating_first_day())
    }
}

impl Month {
    fn saturating_first_month_day(date: NaiveDate) -> NaiveDate {
        date.with_day(1).unwrap_or(NaiveDate::MIN)
    }

    pub fn saturating_first_day(&self) -> NaiveDate {
        Self::saturating_first_month_day(self.date_in_month)
    }
}

impl Timespan for Month {
    fn from_date(date: NaiveDate) -> Self {
        Self {
            date_in_month: Self::saturating_first_month_day(date),
        }
    }

    fn into_date(self) -> NaiveDate {
        Self::saturating_first_month_day(self.date_in_month)
    }

    fn enum_variant() -> ResolutionKind {
        ResolutionKind::Month
    }

    fn saturating_start_timestamp(&self) -> DateTime<Utc> {
        self.saturating_first_day().saturating_start_timestamp()
    }

    fn checked_add(&self, duration: TimespanDuration<Self>) -> Option<Self>
    where
        Self: Sized,
    {
        let duration = chrono::Months::new(duration.repeats().try_into().ok()?);
        self.date_in_month
            .checked_add_months(duration)
            .map(Self::from_date)
    }

    fn checked_sub(&self, duration: TimespanDuration<Self>) -> Option<Self>
    where
        Self: Sized,
    {
        let duration = chrono::Months::new(duration.repeats().try_into().ok()?);
        self.date_in_month
            .checked_sub_months(duration)
            .map(Self::from_date)
    }

    fn max() -> Self {
        Self::from_date(NaiveDate::MAX)
    }

    fn min() -> Self {
        Self::from_date(NaiveDate::MIN)
    }
}

impl ConsistsOf<NaiveDate> for Month {
    fn from_smaller(date: NaiveDate) -> Self {
        Month::from_date(date)
    }

    fn into_smaller(self) -> NaiveDate {
        Month::into_date(self)
    }
}

impl_into_string_timespan_value!(Month, i64);
impl_into_string_timespan_value!(Month, f64);
impl_into_string_timespan_value!(Month, Decimal);

#[cfg(test)]
mod tests {
    use crate::{
        tests::point_construction::{d, dt},
        utils::day_start,
    };

    use super::*;

    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn month_date_conversion_works() {
        assert_eq!(
            Month::from_date(d("2015-01-01")).into_date(),
            d("2015-01-01")
        );
        assert_eq!(
            Month::from_date(d("2015-01-31")).into_date(),
            d("2015-01-01")
        );
        assert_eq!(
            Month::from_date(d("2012-02-29")).into_date(),
            d("2012-02-01")
        );
        assert_eq!(Month::from_date(NaiveDate::MIN).into_date(), NaiveDate::MIN);
        assert_eq!(
            Month::from_date(NaiveDate::MAX).into_date(),
            NaiveDate::MAX.with_day0(0).unwrap()
        );
    }

    #[test]
    fn month_eq_works() {
        assert_eq!(
            Month::from_date(d("2015-01-01")),
            Month::from_date(d("2015-01-01")),
        );
        assert_eq!(
            Month::from_date(d("2015-01-01")),
            Month::from_date(d("2015-01-31")),
        );
        assert_ne!(
            Month::from_date(d("2015-01-01")),
            Month::from_date(d("2015-02-01")),
        );
        assert_ne!(
            Month::from_date(d("2015-01-01")),
            Month::from_date(d("2014-12-31")),
        );
        assert_eq!(
            Month::from_date(d("2012-02-01")),
            Month::from_date(d("2012-02-29")),
        );
        assert_eq!(
            Month::from_date(NaiveDate::MAX),
            Month::from_date(NaiveDate::MAX),
        );
        assert_eq!(
            Month::from_date(NaiveDate::MAX),
            Month::from_date(NaiveDate::MAX.with_day0(0).unwrap()),
        );
        assert_eq!(
            Month::from_date(NaiveDate::MIN),
            Month::from_date(NaiveDate::MIN),
        );
        assert_ne!(
            Month::from_date(NaiveDate::MIN),
            Month::from_date(
                NaiveDate::MIN
                    .with_day0(0)
                    .unwrap()
                    .checked_add_months(chrono::Months::new(1))
                    .unwrap()
            ),
        );
    }

    #[test]
    fn month_saturating_first_day_works() {
        assert_eq!(
            Month::from_date(d("2015-01-01")).saturating_first_day(),
            d("2015-01-01")
        );
        assert_eq!(
            Month::from_date(d("2015-01-02")).saturating_first_day(),
            d("2015-01-01")
        );
        assert_eq!(
            Month::from_date(d("2015-01-31")).saturating_first_day(),
            d("2015-01-01")
        );
        assert_eq!(
            Month::from_date(NaiveDate::MAX).saturating_first_day(),
            NaiveDate::MAX.with_day0(0).unwrap()
        );
        // saturation works
        assert_eq!(
            Month::from_date(NaiveDate::MIN).saturating_first_day(),
            NaiveDate::MIN
        );
    }

    #[test]
    fn month_arithmetics_works() {
        assert_eq!(
            Month::from_date(d("2015-06-01"))
                .saturating_add(TimespanDuration::from_timespan_repeats(3)),
            Month::from_date(d("2015-09-01"))
        );
        assert_eq!(
            Month::from_date(d("2015-06-01"))
                .saturating_sub(TimespanDuration::from_timespan_repeats(3)),
            Month::from_date(d("2015-03-01"))
        );
        assert_eq!(
            Month::from_date(d("2015-06-01"))
                .checked_add(TimespanDuration::from_timespan_repeats(3)),
            Some(Month::from_date(d("2015-09-01")))
        );
        assert_eq!(
            Month::from_date(d("2015-06-01"))
                .checked_sub(TimespanDuration::from_timespan_repeats(3)),
            Some(Month::from_date(d("2015-03-01")))
        );

        assert_eq!(
            Month::from_date(d("2015-06-01"))
                .saturating_add(TimespanDuration::from_timespan_repeats(u64::MAX)),
            Month::from_date(NaiveDate::MAX)
        );
        assert_eq!(
            Month::from_date(d("2015-06-01"))
                .saturating_sub(TimespanDuration::from_timespan_repeats(u64::MAX)),
            Month::from_date(NaiveDate::MIN)
        );

        assert_eq!(
            Month::from_date(d("2015-06-01"))
                .checked_add(TimespanDuration::from_timespan_repeats(u64::MAX)),
            None
        );
        assert_eq!(
            Month::from_date(d("2015-06-01"))
                .checked_sub(TimespanDuration::from_timespan_repeats(u64::MAX)),
            None
        );

        assert_eq!(
            Month::from_date(NaiveDate::MAX)
                .saturating_add(TimespanDuration::from_timespan_repeats(1)),
            Month::from_date(NaiveDate::MAX)
        );
        assert_eq!(
            Month::from_date(NaiveDate::MIN)
                .saturating_sub(TimespanDuration::from_timespan_repeats(1)),
            Month::from_date(NaiveDate::MIN)
        );

        assert_eq!(
            Month::from_date(NaiveDate::MAX)
                .checked_add(TimespanDuration::from_timespan_repeats(1)),
            None
        );
        assert_eq!(
            Month::from_date(NaiveDate::MIN)
                .checked_sub(TimespanDuration::from_timespan_repeats(1)),
            None
        );
    }

    #[test]
    fn month_saturating_first_timestamp_works() {
        assert_eq!(
            Month::from_date(d("2015-01-01")).saturating_start_timestamp(),
            dt("2015-01-01T00:00:00").and_utc()
        );
        assert_eq!(
            Month::from_date(d("2015-12-31")).saturating_start_timestamp(),
            dt("2015-12-01T00:00:00").and_utc()
        );
        assert_eq!(
            Month::from_date(NaiveDate::MAX).saturating_start_timestamp(),
            day_start(&NaiveDate::MAX.with_day0(0).unwrap())
        );
        assert_eq!(
            Month::from_date(NaiveDate::MIN).saturating_start_timestamp(),
            DateTime::<Utc>::MIN_UTC
        );
    }
}
