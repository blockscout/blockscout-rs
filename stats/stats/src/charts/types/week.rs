use std::ops::RangeInclusive;

use chrono::{NaiveDate, NaiveWeek, Weekday};
use entity::chart_data;
use rust_decimal::Decimal;
use sea_orm::{FromQueryResult, Set, TryGetable};

use super::{TimespanValue, ZeroTimespanValue};

pub const WEEK_START: Weekday = Weekday::Mon;

#[derive(Debug)]
pub struct Week(NaiveWeek);

// `NaiveWeek` doesn't implement commong traits, we have to do it by hand

impl PartialEq for Week {
    fn eq(&self, other: &Self) -> bool {
        self.0.first_day().eq(&other.0.first_day())
    }
}

impl Eq for Week {}

impl PartialOrd for Week {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.first_day().partial_cmp(&other.0.first_day())
    }
}

impl Ord for Week {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.first_day().cmp(&other.0.first_day())
    }
}

impl Clone for Week {
    fn clone(&self) -> Self {
        Self::new(self.0.first_day())
    }
}

impl TryGetable for Week {
    fn try_get_by<I: sea_orm::ColIdx>(
        res: &sea_orm::QueryResult,
        index: I,
    ) -> Result<Self, sea_orm::TryGetError> {
        NaiveDate::try_get_by(res, index).map(|d| Week::new(d))
    }
}

impl Week {
    pub fn new(day: NaiveDate) -> Week {
        Self(day.week(WEEK_START))
    }

    pub fn days(&self) -> RangeInclusive<NaiveDate> {
        self.0.days()
    }
}

macro_rules! impl_week_value_decomposition {
    ($name: ident, $val_type:ty) => {
        impl TimespanValue for $name {
            type Timespan = Week;
            type Value = $val_type;
            fn get_parts(&self) -> (&Week, &Self::Value) {
                (&self.week, &self.value)
            }
            fn into_parts(self) -> (Week, Self::Value) {
                (self.week, self.value)
            }
            fn from_parts(week: Week, value: Self::Value) -> Self {
                Self { week, value }
            }
        }
    };
}

#[derive(FromQueryResult, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WeekValueString {
    pub week: Week,
    pub value: String,
}

impl_week_value_decomposition!(WeekValueString, String);

impl ZeroTimespanValue for WeekValueString {
    fn with_zero_value(week: Week) -> Self {
        Self {
            week,
            value: "0".to_string(),
        }
    }
}

impl WeekValueString {
    pub fn active_model(
        &self,
        chart_id: i32,
        min_blockscout_block: Option<i64>,
    ) -> chart_data::ActiveModel {
        chart_data::ActiveModel {
            id: Default::default(),
            chart_id: Set(chart_id),
            date: Set(self.week.days().start().clone()),
            value: Set(self.value.clone()),
            created_at: Default::default(),
            min_blockscout_block: Set(min_blockscout_block),
        }
    }
}

/// Implement non-string week-value type
macro_rules! create_week_value_with {
    ($name:ident, $val_type:ty) => {
        #[derive(FromQueryResult, Debug, Clone, PartialEq)]
        pub struct $name {
            pub week: Week,
            pub value: $val_type,
        }

        impl_week_value_decomposition!($name, $val_type);

        impl From<$name> for WeekValueString {
            fn from(value: $name) -> Self {
                Self {
                    week: value.week,
                    value: value.value.to_string(),
                }
            }
        }
    };
}

create_week_value_with!(WeekValueInt, i64);
create_week_value_with!(WeekValueDouble, f64);
create_week_value_with!(WeekValueDecimal, Decimal);

impl ZeroTimespanValue for WeekValueInt {
    fn with_zero_value(week: Week) -> Self {
        Self { week, value: 0 }
    }
}

impl ZeroTimespanValue for WeekValueDouble {
    fn with_zero_value(week: Week) -> Self {
        Self { week, value: 0.0 }
    }
}

impl ZeroTimespanValue for WeekValueDecimal {
    fn with_zero_value(week: Week) -> Self {
        Self {
            week,
            value: 0.into(),
        }
    }
}
