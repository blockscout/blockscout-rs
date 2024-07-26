use chrono::{DateTime, Datelike, NaiveDate, Utc};

use crate::{
    types::{Timespan, TimespanDuration},
    ResolutionKind,
};

/// Year number within range of `chrono::NaiveDate`
#[derive(Copy, Clone)]
pub struct Year(i32);

impl Year {
    /// First day of the year or `NaiveDate::MIN`
    pub fn saturating_first_day(&self) -> NaiveDate {
        NaiveDate::from_yo_opt(self.0, 1).unwrap_or(NaiveDate::MIN)
    }

    pub fn clamp_by_naive_date_range(self) -> Self {
        if let Some(_) = NaiveDate::from_yo_opt(self.0, 1) {
            self
        } else {
            if self.0 > 0 {
                Self::from_date(NaiveDate::MAX)
            } else {
                Self::from_date(NaiveDate::MIN)
            }
        }
    }
}

impl Timespan for Year {
    fn from_date(date: NaiveDate) -> Self {
        Self(date.year())
    }

    fn into_date(self) -> NaiveDate {
        self.saturating_first_day()
    }

    fn saturating_next_timespan(&self) -> Self {
        // always ensure we stay within `NaiveDate`
        // first clamp to make sure we don't stay on max value if value is too large
        let self_clamped = self.clamp_by_naive_date_range();
        let next = Self(self_clamped.0.saturating_add(1));
        next.clamp_by_naive_date_range()
    }

    fn saturating_previous_timespan(&self) -> Self {
        // always ensure we stay within `NaiveDate`
        // first clamp to make sure we don't stay on min value if value is too small
        let self_clamped = self.clamp_by_naive_date_range();
        let prev = Self(self_clamped.0.saturating_sub(1));
        prev.clamp_by_naive_date_range()
    }

    fn enum_variant() -> ResolutionKind {
        ResolutionKind::Year
    }

    fn start_timestamp(&self) -> DateTime<Utc> {
        self.saturating_first_day().start_timestamp()
    }

    fn saturating_add(&self, duration: TimespanDuration<Self>) -> Self
    where
        Self: Sized,
    {
        let add_years = duration.repeats().try_into().unwrap_or(std::i32::MAX);
        Self(self.0.saturating_add(add_years)).clamp_by_naive_date_range()
    }

    fn saturating_sub(&self, duration: TimespanDuration<Self>) -> Self
    where
        Self: Sized,
    {
        let sub_years: i32 = duration.repeats().try_into().unwrap_or(std::i32::MAX);
        Self(self.0.saturating_sub(sub_years)).clamp_by_naive_date_range()
    }
}
