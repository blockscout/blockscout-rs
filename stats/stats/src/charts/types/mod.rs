mod day;
pub mod week;

pub use day::*;

/// Any type that represents some value for
/// some time interval.
pub trait TimespanValue {
    type Timespan;
    type Value;
    fn get_parts(&self) -> (&Self::Timespan, &Self::Value);
    fn into_parts(self) -> (Self::Timespan, Self::Value);
    fn from_parts(timespan: Self::Timespan, value: Self::Value) -> Self;
}

pub trait ZeroTimespanValue: TimespanValue + Sized
where
    Self::Timespan: PartialOrd,
{
    fn with_zero_value(timespan: Self::Timespan) -> Self;

    fn relevant_or_zero(self, current_interval: Self::Timespan) -> Self {
        if self.get_parts().0 < &current_interval {
            Self::with_zero_value(current_interval)
        } else {
            self
        }
    }
}
