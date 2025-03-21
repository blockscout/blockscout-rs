use chrono::NaiveDate;
use sea_orm::FromQueryResult;

use crate::{data_source::kinds::data_manipulation::map::MapFunction, ChartError};

use super::{TimespanTrait, TimespanValue, ZeroTimespanValue};

#[derive(FromQueryResult, Clone, Debug, PartialEq)]
pub struct NewTxnsCombinedPoint {
    pub date: NaiveDate,
    pub all_transactions: String,
    pub op_stack_operational_transactions: String,
}

impl TimespanTrait for NewTxnsCombinedPoint {
    type Timespan = NaiveDate;

    fn timespan(&self) -> &Self::Timespan {
        &self.date
    }

    fn timespan_mut(&mut self) -> &mut Self::Timespan {
        &mut self.date
    }
}

impl ZeroTimespanValue<NaiveDate> for NewTxnsCombinedPoint {
    fn with_zero_value(timespan: Self::Timespan) -> Self {
        Self {
            date: timespan,
            all_transactions: "0".to_string(),
            op_stack_operational_transactions: "0".to_string(),
        }
    }
}

pub struct ExtractAllTxns;

impl MapFunction<Vec<NewTxnsCombinedPoint>> for ExtractAllTxns {
    type Output = Vec<TimespanValue<NaiveDate, String>>;

    fn function(inner_data: Vec<NewTxnsCombinedPoint>) -> Result<Self::Output, ChartError> {
        Ok(inner_data
            .into_iter()
            .map(|p| TimespanValue {
                timespan: p.date,
                value: p.all_transactions,
            })
            .collect())
    }
}

impl MapFunction<NewTxnsCombinedPoint> for ExtractAllTxns {
    type Output = TimespanValue<NaiveDate, String>;

    fn function(inner_data: NewTxnsCombinedPoint) -> Result<Self::Output, ChartError> {
        Ok(TimespanValue {
            timespan: inner_data.date,
            value: inner_data.all_transactions,
        })
    }
}

pub struct ExtractOpStackTxns;

impl MapFunction<Vec<NewTxnsCombinedPoint>> for ExtractOpStackTxns {
    type Output = Vec<TimespanValue<NaiveDate, String>>;

    fn function(inner_data: Vec<NewTxnsCombinedPoint>) -> Result<Self::Output, ChartError> {
        Ok(inner_data
            .into_iter()
            .map(|p| TimespanValue {
                timespan: p.date,
                value: p.op_stack_operational_transactions,
            })
            .collect())
    }
}
impl MapFunction<NewTxnsCombinedPoint> for ExtractOpStackTxns {
    type Output = TimespanValue<NaiveDate, String>;

    fn function(inner_data: NewTxnsCombinedPoint) -> Result<Self::Output, ChartError> {
        Ok(TimespanValue {
            timespan: inner_data.date,
            value: inner_data.op_stack_operational_transactions,
        })
    }
}
