#![cfg(test)]

use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;

use crate::{
    charts::types::DateValue,
    types::week::{Week, WeekValue},
};

pub fn week_of(date: &str) -> Week {
    Week::new(d(date))
}

pub fn d(date: &str) -> NaiveDate {
    date.parse().unwrap()
}

pub fn dt(time: &str) -> NaiveDateTime {
    time.parse().unwrap()
}

pub fn d_v(date: &str, value: &str) -> DateValue<String> {
    DateValue::<String> {
        timespan: d(date),
        value: value.to_string(),
    }
}

pub fn d_v_int(date: &str, value: i64) -> DateValue<i64> {
    DateValue::<i64> {
        timespan: d(date),
        value,
    }
}

pub fn d_v_decimal(date: &str, value: Decimal) -> DateValue<Decimal> {
    DateValue::<Decimal> {
        timespan: d(date),
        value,
    }
}

pub fn d_v_double(date: &str, value: f64) -> DateValue<f64> {
    DateValue::<f64> {
        timespan: d(date),
        value,
    }
}

pub fn week_v(week_of_date: &str, value: &str) -> WeekValue<String> {
    WeekValue::<String> {
        timespan: week_of(week_of_date),
        value: value.to_string(),
    }
}

pub fn week_v_int(week_of_date: &str, value: i64) -> WeekValue<i64> {
    WeekValue::<i64> {
        timespan: week_of(week_of_date),
        value,
    }
}

pub fn week_v_decimal(week_of_date: &str, value: Decimal) -> WeekValue<Decimal> {
    WeekValue::<Decimal> {
        timespan: week_of(week_of_date),
        value,
    }
}

pub fn week_v_double(week_of_date: &str, value: f64) -> WeekValue<f64> {
    WeekValue::<f64> {
        timespan: week_of(week_of_date),
        value,
    }
}
