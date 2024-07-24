use chrono::{Days, NaiveDate};

use sea_orm::{prelude::*, FromQueryResult, TryGetable};

use crate::{
    charts::chart_properties_portrait::imports::ResolutionKind, impl_into_string_timespan_value,
    utils::day_start,
};

use super::{db::DbDateValue, TimespanValue};

impl<V: TryGetable> FromQueryResult for TimespanValue<NaiveDate, V> {
    fn from_query_result(res: &QueryResult, pre: &str) -> Result<Self, DbErr> {
        DbDateValue::<V>::from_query_result(res, pre).map(|dv| dv.into())
    }
}

pub type DateValue<V> = TimespanValue<NaiveDate, V>;

impl super::Timespan for NaiveDate {
    fn from_date(date: NaiveDate) -> Self {
        date
    }

    fn into_date(self) -> NaiveDate {
        self
    }

    fn saturating_next_timespan(&self) -> Self {
        self.checked_add_days(Days::new(1))
            .unwrap_or(NaiveDate::MAX)
    }

    fn saturating_previous_timespan(&self) -> Self {
        self.checked_sub_days(Days::new(1))
            .unwrap_or(NaiveDate::MIN)
    }

    fn enum_variant() -> ResolutionKind {
        ResolutionKind::Day
    }

    fn start_timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        day_start(self)
    }

    fn add_duration(&self, duration: super::TimespanDuration<Self>) -> Self
    where
        Self: Sized,
    {
        self.checked_add_days(Days::new(duration.repeats()))
            .unwrap_or(NaiveDate::MAX)
    }

    fn sub_duration(&self, duration: super::TimespanDuration<Self>) -> Self
    where
        Self: Sized,
    {
        self.checked_sub_days(Days::new(duration.repeats()))
            .unwrap_or(NaiveDate::MIN)
    }
}

impl_into_string_timespan_value!(NaiveDate, i64);
impl_into_string_timespan_value!(NaiveDate, f64);
impl_into_string_timespan_value!(NaiveDate, Decimal);
