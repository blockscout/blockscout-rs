#![cfg(test)]

use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;

use crate::{
    charts::types::TimespanValue,
    types::{
        timespans::{Month, Week, Year},
        Timespan,
    },
};

pub fn d(date: &str) -> NaiveDate {
    date.parse().unwrap()
}

pub fn dt(time: &str) -> NaiveDateTime {
    time.parse().unwrap()
}

pub fn week_of(date: &str) -> Week {
    Week::new(d(date))
}

pub fn month_of(date: &str) -> Month {
    Month::from_date(d(date))
}

pub fn year_of(date: &str) -> Year {
    Year::from_date(d(date))
}

macro_rules! impl_timespan_value_test_definitions {
    (
        $type:ty,
        $convert_date_str_to_timespan:ident,
        $string_name:ident,
        $int_name:ident,
        $double_name:ident,
        $decimal_name:ident $(,)?
    ) => {
        /// Timespan that corresponds to the `date` will be taken
        pub fn $string_name(date: &str, value: &str) -> TimespanValue<$type, String> {
            TimespanValue::<$type, String> {
                timespan: $convert_date_str_to_timespan(date),
                value: value.to_string(),
            }
        }

        /// Timespan that corresponds to the `date` will be taken
        pub fn $int_name(date: &str, value: i64) -> TimespanValue<$type, i64> {
            TimespanValue::<$type, i64> {
                timespan: $convert_date_str_to_timespan(date),
                value,
            }
        }

        /// Timespan that corresponds to the `date` will be taken
        pub fn $double_name(date: &str, value: f64) -> TimespanValue<$type, f64> {
            TimespanValue::<$type, f64> {
                timespan: $convert_date_str_to_timespan(date),
                value,
            }
        }

        /// Timespan that corresponds to the `date` will be taken
        pub fn $decimal_name(date: &str, value: Decimal) -> TimespanValue<$type, Decimal> {
            TimespanValue::<$type, Decimal> {
                timespan: $convert_date_str_to_timespan(date),
                value,
            }
        }
    };
}

impl_timespan_value_test_definitions!(NaiveDate, d, d_v, d_v_int, d_v_double, d_v_decimal);
impl_timespan_value_test_definitions!(Week, week_of, w_v, w_v_int, w_v_double, w_v_decimal);
impl_timespan_value_test_definitions!(Month, month_of, m_v, m_v_int, m_v_double, m_v_decimal);
impl_timespan_value_test_definitions!(Year, year_of, y_v, y_v_int, y_v_double, y_v_decimal);
