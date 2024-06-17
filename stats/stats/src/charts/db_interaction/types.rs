use chrono::NaiveDate;
use entity::chart_data;

use sea_orm::{prelude::*, FromQueryResult, Set};

pub trait DateValue {
    type Value;
    fn get_parts(&self) -> (&NaiveDate, &Self::Value);
    fn into_parts(self) -> (NaiveDate, Self::Value);
    fn from_parts(date: NaiveDate, value: Self::Value) -> Self;
}

macro_rules! impl_date_value_decomposition {
    ($name: ident, $val_type:ty) => {
        impl DateValue for $name {
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

/// Implement non-base date-value type
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

#[derive(FromQueryResult, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateValueString {
    pub date: NaiveDate,
    pub value: String,
}

impl_date_value_decomposition!(DateValueString, String);

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

pub trait ZeroDateValue: DateValue + Sized {
    fn with_zero_value(date: NaiveDate) -> Self;

    fn relevant_or_zero(self, current_date: NaiveDate) -> Self {
        if self.get_parts().0 < &current_date {
            Self::with_zero_value(current_date)
        } else {
            self
        }
    }
}

impl ZeroDateValue for DateValueString {
    fn with_zero_value(date: NaiveDate) -> Self {
        Self {
            date,
            value: "0".to_string(),
        }
    }
}

impl ZeroDateValue for DateValueInt {
    fn with_zero_value(date: NaiveDate) -> Self {
        Self { date, value: 0 }
    }
}

impl ZeroDateValue for DateValueDouble {
    fn with_zero_value(date: NaiveDate) -> Self {
        Self { date, value: 0.0 }
    }
}

impl ZeroDateValue for DateValueDecimal {
    fn with_zero_value(date: NaiveDate) -> Self {
        Self {
            date,
            value: 0.into(),
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
    pub fn from_date_value(dv: DateValueString, is_approximate: bool) -> Self {
        Self {
            date: dv.date,
            value: dv.value,
            is_approximate,
        }
    }
}

impl From<ExtendedDateValue> for DateValueString {
    fn from(dv: ExtendedDateValue) -> Self {
        DateValueString {
            date: dv.date,
            value: dv.value,
        }
    }
}
