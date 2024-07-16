use chrono::{Days, NaiveDate};

use sea_orm::{prelude::*, FromQueryResult, TryGetable};

use crate::{charts::chart_properties_portrait::imports::ResolutionKind, utils::day_start};

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

    fn next_timespan(&self) -> Self {
        self.checked_add_days(Days::new(1))
            .unwrap_or(NaiveDate::MAX)
    }

    fn previous_timespan(&self) -> Self {
        self.checked_sub_days(Days::new(1))
            .unwrap_or(NaiveDate::MIN)
    }

    fn enum_variant() -> ResolutionKind {
        ResolutionKind::Day
    }

    fn start_timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        day_start(self)
    }
}

//todo: remove???

/// Implement non-string date-value type
macro_rules! impl_into_string_date_value {
    ($impl_for:ty) => {
        impl From<$impl_for> for DateValue<String> {
            fn from(value: $impl_for) -> Self {
                Self {
                    timespan: value.timespan,
                    value: value.value.to_string(),
                }
            }
        }
    };
}

impl_into_string_date_value!(DateValue<i64>);
impl_into_string_date_value!(DateValue<f64>);
impl_into_string_date_value!(DateValue<Decimal>);
