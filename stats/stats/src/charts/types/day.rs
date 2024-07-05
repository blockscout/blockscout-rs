use chrono::NaiveDate;
use entity::chart_data;

use sea_orm::{prelude::*, FromQueryResult, Set};

use super::{TimespanValue, ZeroTimespanValue};

// todo: remove the traits probably

pub trait DateValue: TimespanValue<Timespan = NaiveDate> {}

impl<DV> DateValue for DV where DV: TimespanValue<Timespan = NaiveDate> {}

pub trait ZeroDateValue: ZeroTimespanValue<Timespan = NaiveDate> {}

impl<DV> ZeroDateValue for DV where DV: ZeroTimespanValue<Timespan = NaiveDate> {}

macro_rules! impl_date_value_decomposition {
    ($name: ident, $val_type:ty) => {
        impl TimespanValue for $name {
            type Timespan = NaiveDate;
            type Value = $val_type;
            fn get_parts(&self) -> (&NaiveDate, &Self::Value) {
                (&self.date, &self.value)
            }
            fn into_parts(self) -> (NaiveDate, Self::Value) {
                (self.date, self.value)
            }
            fn from_parts(date: NaiveDate, value: Self::Value) -> Self {
                Self { date, value }
            }
        }
    };
}

#[derive(FromQueryResult, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateValueString {
    pub date: NaiveDate,
    pub value: String,
}

impl_date_value_decomposition!(DateValueString, String);

impl ZeroTimespanValue for DateValueString {
    fn with_zero_value(date: NaiveDate) -> Self {
        Self {
            date,
            value: "0".to_string(),
        }
    }
}

impl DateValueString {
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
}

/// Implement non-string date-value type
macro_rules! create_date_value_with {
    ($name:ident, $val_type:ty) => {
        #[derive(FromQueryResult, Debug, Clone, Default, PartialEq)]
        pub struct $name {
            pub date: NaiveDate,
            pub value: $val_type,
        }

        impl_date_value_decomposition!($name, $val_type);

        impl From<$name> for DateValueString {
            fn from(value: $name) -> Self {
                Self {
                    date: value.date,
                    value: value.value.to_string(),
                }
            }
        }
    };
}

create_date_value_with!(DateValueInt, i64);
create_date_value_with!(DateValueDouble, f64);
create_date_value_with!(DateValueDecimal, Decimal);

impl ZeroTimespanValue for DateValueInt {
    fn with_zero_value(date: NaiveDate) -> Self {
        Self { date, value: 0 }
    }
}

impl ZeroTimespanValue for DateValueDouble {
    fn with_zero_value(date: NaiveDate) -> Self {
        Self { date, value: 0.0 }
    }
}

impl ZeroTimespanValue for DateValueDecimal {
    fn with_zero_value(date: NaiveDate) -> Self {
        Self {
            date,
            value: 0.into(),
        }
    }
}

/// Marked as precise or approximate
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtendedDateValueString {
    pub date: NaiveDate,
    pub value: String,
    pub is_approximate: bool,
}

impl ExtendedDateValueString {
    pub fn from_date_value(dv: DateValueString, is_approximate: bool) -> Self {
        Self {
            date: dv.date,
            value: dv.value,
            is_approximate,
        }
    }
}

impl From<ExtendedDateValueString> for DateValueString {
    fn from(dv: ExtendedDateValueString) -> Self {
        DateValueString {
            date: dv.date,
            value: dv.value,
        }
    }
}
