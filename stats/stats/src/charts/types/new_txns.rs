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
        Ok(inner_data.into_iter().map(extract_all_txns_inner).collect())
    }
}

impl MapFunction<NewTxnsCombinedPoint> for ExtractAllTxns {
    type Output = TimespanValue<NaiveDate, String>;

    fn function(inner_data: NewTxnsCombinedPoint) -> Result<Self::Output, ChartError> {
        Ok(extract_all_txns_inner(inner_data))
    }
}

fn extract_all_txns_inner(combined: NewTxnsCombinedPoint) -> TimespanValue<NaiveDate, String> {
    TimespanValue {
        timespan: combined.date,
        value: combined.all_transactions,
    }
}

pub struct ExtractOpStackTxns;

impl MapFunction<Vec<NewTxnsCombinedPoint>> for ExtractOpStackTxns {
    type Output = Vec<TimespanValue<NaiveDate, String>>;

    fn function(inner_data: Vec<NewTxnsCombinedPoint>) -> Result<Self::Output, ChartError> {
        Ok(inner_data
            .into_iter()
            .map(extract_op_stack_operational_inner)
            .collect())
    }
}
impl MapFunction<NewTxnsCombinedPoint> for ExtractOpStackTxns {
    type Output = TimespanValue<NaiveDate, String>;

    fn function(inner_data: NewTxnsCombinedPoint) -> Result<Self::Output, ChartError> {
        Ok(extract_op_stack_operational_inner(inner_data))
    }
}

fn extract_op_stack_operational_inner(
    combined: NewTxnsCombinedPoint,
) -> TimespanValue<NaiveDate, String> {
    TimespanValue {
        timespan: combined.date,
        value: combined.op_stack_operational_transactions,
    }
}
