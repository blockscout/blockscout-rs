#![cfg(test)]

use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;

use crate::{
    charts::types::{DateValueDecimal, DateValueInt},
    types::{
        week::{Week, WeekValueDouble},
        DateValueDouble,
    },
    DateValueString,
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

pub fn v(date: &str, value: &str) -> DateValueString {
    DateValueString {
        date: d(date),
        value: value.to_string(),
    }
}

pub fn v_int(date: &str, value: i64) -> DateValueInt {
    DateValueInt {
        date: d(date),
        value,
    }
}

pub fn v_decimal(date: &str, value: Decimal) -> DateValueDecimal {
    DateValueDecimal {
        date: d(date),
        value,
    }
}

pub fn v_double(date: &str, value: f64) -> DateValueDouble {
    DateValueDouble {
        date: d(date),
        value,
    }
}

pub fn week_v_double(week_of_date: &str, value: f64) -> WeekValueDouble {
    WeekValueDouble {
        week: week_of(week_of_date),
        value,
    }
}
