use std::ops::Range;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::charts::ResolutionKind;

use super::{week::Week, TimespanDuration, TimespanValue};

pub trait Timespan {
    /// Construct the timespan from a date within the timespan.
    ///
    /// Note that `from` is not a reversible function.
    /// I.e. for some date `d`, `Timespan::from_date(d).into_date() != d`
    fn from_date(date: NaiveDate) -> Self;
    /// Convert the timespan into a corresponding date
    /// to store in database.
    fn into_date(self) -> NaiveDate;
    /// Get the next interval right after the current one (saturating)
    fn saturating_next_timespan(&self) -> Self;
    /// Get the interval right before the current one (saturating)
    fn saturating_previous_timespan(&self) -> Self;
    /// Converting type into runtime enum variant
    fn enum_variant() -> ResolutionKind;
    /// Extract the start of given timespan as UTC timestamp
    fn start_timestamp(&self) -> DateTime<Utc>;
    /// Represent the timespan as UTC timestamp range
    fn into_time_range(self) -> Range<DateTime<Utc>>
    where
        Self: Sized,
    {
        self.start_timestamp()..self.saturating_next_timespan().start_timestamp()
    }
    fn add_duration(&self, duration: TimespanDuration<Self>) -> Self
    where
        Self: Sized;
    fn sub_duration(&self, duration: TimespanDuration<Self>) -> Self
    where
        Self: Sized;
}

// if for some rare reason trait is needed
// (currently only for `ZeroTimespanValue` impl)
pub trait TimespanValueTrait {
    type Timespan;
    type Value;

    fn parts(&self) -> (&Self::Timespan, &Self::Value);
    fn into_parts(self) -> (Self::Timespan, Self::Value);
    fn from_parts(t: Self::Timespan, v: Self::Value) -> Self;
}

impl<T, V> TimespanValueTrait for TimespanValue<T, V> {
    type Timespan = T;
    type Value = V;

    fn parts(&self) -> (&Self::Timespan, &Self::Value) {
        (&self.timespan, &self.value)
    }

    fn into_parts(self) -> (Self::Timespan, Self::Value) {
        (self.timespan, self.value)
    }

    fn from_parts(t: Self::Timespan, v: Self::Value) -> Self {
        Self {
            timespan: t,
            value: v,
        }
    }
}

// generic to unify the parameters
pub trait ZeroTimespanValue<T>: TimespanValueTrait<Timespan = T> + Sized
where
    Self::Timespan: Ord,
{
    fn with_zero_value(timespan: Self::Timespan) -> Self;

    fn relevant_or_zero(self, current_timespan: Self::Timespan) -> Self {
        if self.parts().0 < &current_timespan {
            Self::with_zero_value(current_timespan)
        } else {
            self
        }
    }
}

// https://github.com/rust-lang/rfcs/issues/2758
macro_rules! impl_zero_timespan_value_for_zero_value {
    ($value:ty) => {
        impl<T: Ord> ZeroTimespanValue<T> for TimespanValue<T, $value> {
            fn with_zero_value(timespan: T) -> Self {
                Self {
                    timespan,
                    value: <$value as ::rust_decimal::prelude::Zero>::zero(),
                }
            }
        }
    };
}

impl_zero_timespan_value_for_zero_value!(i64);
impl_zero_timespan_value_for_zero_value!(f64);
impl_zero_timespan_value_for_zero_value!(Decimal);

impl<T: Ord> ZeroTimespanValue<T> for TimespanValue<T, String> {
    fn with_zero_value(timespan: T) -> Self {
        Self {
            timespan,
            value: "0".to_string(),
        }
    }
}

/// Indicates that this timespan can be broken into (multiple) `SmallerTimespan`s.
///
/// Examples (types may not be called exactly like this):
/// - `Week: ConsistsOf<Day>`
/// - `Month: ConsistsOf<Day>`
/// - `Year: ConsistsOf<Day>`
/// - `Year: ConsistsOf<Month>`
///
/// but not `Year: ConsistsOf<Week>` or `Month: ConsistsOf<Week>` because week borders might not align
/// with years'/months' borders.
pub trait ConsistsOf<SmallerTimespan: Timespan> {
    /// Construct the timespan from a date within the timespan.
    ///
    /// Note that `from` is not a reversible function.
    /// I.e. for some date `d`, `Timespan::from_date(d).into_date() != d`
    fn from_smaller(date: SmallerTimespan) -> Self;
    /// Convert the timespan into a corresponding date
    /// to store in database.
    fn into_smaller(self) -> SmallerTimespan;
}

impl ConsistsOf<NaiveDate> for Week {
    fn from_smaller(date: NaiveDate) -> Self {
        Week::from_date(date)
    }

    fn into_smaller(self) -> NaiveDate {
        Week::into_date(self)
    }
}
