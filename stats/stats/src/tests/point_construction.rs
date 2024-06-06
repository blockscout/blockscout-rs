#![cfg(test)]

use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;

use crate::{
    charts::db_interaction::types::{DateValueDecimal, DateValueInt},
    DateValueString,
};

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
