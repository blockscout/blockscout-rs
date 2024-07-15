mod day;
pub mod db;
pub mod week;

use chrono::NaiveDate;
pub use day::*;
use entity::chart_data;
use rust_decimal::Decimal;
use sea_orm::Set;

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
    fn next_timespan(self) -> Self;
}

/// Some value for some time interval
#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimespanValue<T, V> {
    pub timespan: T,
    pub value: V,
}

impl<T> TimespanValue<T, String> {
    pub fn with_zero_value(timespan: T) -> Self {
        Self {
            timespan,
            value: "0".to_string(),
        }
    }
}

impl<T: Ord> TimespanValue<T, String> {
    pub fn relevant_or_zero(self, current_timespan: T) -> Self {
        if self.timespan < current_timespan {
            Self::with_zero_value(current_timespan)
        } else {
            self
        }
    }
}

impl<T: Timespan + Clone> TimespanValue<T, String> {
    pub fn active_model(
        &self,
        chart_id: i32,
        min_blockscout_block: Option<i64>,
    ) -> chart_data::ActiveModel {
        chart_data::ActiveModel {
            id: Default::default(),
            chart_id: Set(chart_id),
            date: Set(self.timespan.clone().into_date()),
            value: Set(self.value.clone()),
            created_at: Default::default(),
            min_blockscout_block: Set(min_blockscout_block),
        }
    }
}

// if for some rare reason trait is needed
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

/// Marked as precise or approximate
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtendedTimespanValue<T, V> {
    // todo: rename
    pub date: T,
    pub value: V,
    pub is_approximate: bool,
}

impl<T, V> ExtendedTimespanValue<T, V> {
    pub fn from_date_value(dv: TimespanValue<T, V>, is_approximate: bool) -> Self {
        Self {
            date: dv.timespan,
            value: dv.value,
            is_approximate,
        }
    }
}

impl<T, V> From<ExtendedTimespanValue<T, V>> for TimespanValue<T, V> {
    fn from(dv: ExtendedTimespanValue<T, V>) -> Self {
        Self {
            timespan: dv.date,
            value: dv.value,
        }
    }
}
