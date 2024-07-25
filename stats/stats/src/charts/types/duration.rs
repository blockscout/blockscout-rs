use std::marker::PhantomData;

use chrono::NaiveDate;

/// Duration expressed as some timespan `T` repeated
/// `n > 0` times
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimespanDuration<T> {
    repeats: u64,
    timespan: PhantomData<T>,
}

impl<T> TimespanDuration<T> {
    /// Create a duration consisting of timespan `T` repeated
    /// `n` times.
    pub fn timespan_repeats(n: u64) -> Self {
        Self {
            repeats: n,
            timespan: PhantomData,
        }
    }

    pub fn repeats(&self) -> u64 {
        self.repeats
    }
}

impl TimespanDuration<NaiveDate> {
    pub fn days(n: u64) -> Self {
        Self::timespan_repeats(n)
    }
}
