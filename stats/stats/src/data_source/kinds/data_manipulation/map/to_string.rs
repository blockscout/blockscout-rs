use crate::{types::TimespanValue, ChartError};

use super::{Map, MapFunction};

pub struct ToStringFunction;

impl<Resolution, Value> MapFunction<Vec<TimespanValue<Resolution, Value>>> for ToStringFunction
where
    Resolution: Send,
    TimespanValue<Resolution, Value>: Into<TimespanValue<Resolution, String>>,
{
    type Output = Vec<TimespanValue<Resolution, String>>;
    fn function(
        inner_data: Vec<TimespanValue<Resolution, Value>>,
    ) -> Result<Self::Output, ChartError> {
        Ok(inner_data.into_iter().map(|p| p.into()).collect())
    }
}

impl<Resolution, Value> MapFunction<TimespanValue<Resolution, Value>> for ToStringFunction
where
    Resolution: Send,
    TimespanValue<Resolution, Value>: Into<TimespanValue<Resolution, String>>,
{
    type Output = TimespanValue<Resolution, String>;
    fn function(inner_data: TimespanValue<Resolution, Value>) -> Result<Self::Output, ChartError> {
        Ok(inner_data.into())
    }
}

/// Convert (numeric) values to strings
pub type MapToString<D> = Map<D, ToStringFunction>;
