use std::ops::Range;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::charts::ResolutionKind;

use super::{TimespanDuration, TimespanValue};

pub trait Timespan {
    /// Construct the timespan from a date within the timespan.
    ///
    /// Note that `from` is not a reversible function.
    /// I.e. for some date `d`, `Timespan::from_date(d).into_date() != d`
    fn from_date(date: NaiveDate) -> Self;
    /// Convert the timespan into a corresponding date
    /// to store in database.
    ///
    /// Using `from_date` on resulting date will always result in equal timespan.
    fn into_date(self) -> NaiveDate;
    /// Get the next interval right after the current one (saturating)
    fn saturating_next_timespan(&self) -> Self
    where
        Self: Sized,
    {
        self.saturating_add(TimespanDuration::from_timespan_repeats(1))
    }
    /// Get the interval right before the current one (saturating)
    fn saturating_previous_timespan(&self) -> Self
    where
        Self: Sized,
    {
        self.saturating_sub(TimespanDuration::from_timespan_repeats(1))
    }
    /// Converting type into runtime enum variant
    fn enum_variant() -> ResolutionKind;
    /// Extract the start of given timespan as UTC timestamp
    fn saturating_start_timestamp(&self) -> DateTime<Utc>;
    /// Represent the timespan as UTC timestamp range
    fn into_time_range(self) -> Range<DateTime<Utc>>
    where
        Self: Sized,
    {
        self.saturating_start_timestamp()
            ..self.saturating_next_timespan().saturating_start_timestamp()
    }
    fn saturating_add(&self, duration: TimespanDuration<Self>) -> Self
    where
        Self: Sized;
    fn saturating_sub(&self, duration: TimespanDuration<Self>) -> Self
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
pub trait ConsistsOf<SmallerTimespan> {
    /// Construct the timespan containing the smaller timespan.
    ///
    /// Note that `from` is usually not a reversible function.
    /// I.e. for some smaller timespan `s`, `T::from_smaller(s).into_smaller() != s`
    /// can be true
    fn from_smaller(smaller: SmallerTimespan) -> Self;
    /// Convert the timespan into a corresponding smaller timespan (contained in `self`).
    fn into_smaller(self) -> SmallerTimespan;
}
