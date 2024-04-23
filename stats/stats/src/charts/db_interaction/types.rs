use std::num::ParseIntError;

use chrono::NaiveDate;
use entity::chart_data;
use sea_orm::{prelude::*, FromQueryResult, Set};

#[derive(FromQueryResult, Debug, Clone)]
pub struct DateValueInt {
    pub date: NaiveDate,
    pub value: i64,
}

impl From<DateValueInt> for DateValue {
    fn from(value: DateValueInt) -> Self {
        Self {
            date: value.date,
            value: value.value.to_string(),
        }
    }
}

impl TryFrom<DateValue> for DateValueInt {
    type Error = ParseIntError;

    fn try_from(value: DateValue) -> Result<Self, Self::Error> {
        Ok(Self {
            date: value.date,
            value: value.value.parse()?,
        })
    }
}

#[derive(FromQueryResult, Debug, Clone)]
pub struct DateValueDouble {
    pub date: NaiveDate,
    pub value: f64,
}

impl From<DateValueDouble> for DateValue {
    fn from(value: DateValueDouble) -> Self {
        Self {
            date: value.date,
            value: value.value.to_string(),
        }
    }
}

#[derive(FromQueryResult, Debug, Clone)]
pub struct DateValueDecimal {
    pub date: NaiveDate,
    pub value: Decimal,
}

impl From<DateValueDecimal> for DateValue {
    fn from(value: DateValueDecimal) -> Self {
        Self {
            date: value.date,
            value: value.value.to_string(),
        }
    }
}

#[derive(FromQueryResult, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateValue {
    pub date: NaiveDate,
    pub value: String,
}

impl DateValue {
    pub fn active_model(
        &self,
        chart_id: i32,
        min_blockscout_block: Option<i64>,
    ) -> chart_data::ActiveModel {
        chart_data::ActiveModel {
            id: Default::default(),
            chart_id: Set(chart_id),
            date: Set(self.date),
            value: Set(self.value.clone()),
            created_at: Default::default(),
            min_blockscout_block: Set(min_blockscout_block),
        }
    }

    pub fn zero(date: NaiveDate) -> Self {
        Self {
            date,
            value: "0".to_string(),
        }
    }

    pub fn relevant_or_zero(self, current_date: NaiveDate) -> DateValue {
        if self.date < current_date {
            DateValue::zero(current_date)
        } else {
            self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtendedDateValue {
    pub date: NaiveDate,
    pub value: String,
    pub is_approximate: bool,
}

impl ExtendedDateValue {
    pub fn from_date_value(dv: DateValue, is_approximate: bool) -> Self {
        Self {
            date: dv.date,
            value: dv.value,
            is_approximate,
        }
    }
}

impl From<ExtendedDateValue> for DateValue {
    fn from(dv: ExtendedDateValue) -> Self {
        DateValue {
            date: dv.date,
            value: dv.value,
        }
    }
}
