use crate::{types::TimespanValue, UpdateError};

use super::{Map, MapFunction};

pub struct ToStringFunction;

impl<Resolution, Value> MapFunction<Vec<TimespanValue<Resolution, Value>>> for ToStringFunction
where
    Resolution: Send,
    Value: Into<String>,
{
    type Output = Vec<TimespanValue<Resolution, String>>;
    fn function(
        inner_data: Vec<TimespanValue<Resolution, Value>>,
    ) -> Result<Self::Output, UpdateError> {
        Ok(inner_data
            .into_iter()
            .map(|p| TimespanValue {
                timespan: p.timespan,
                value: p.value.into(),
            })
            .collect())
    }
}

impl<Resolution, Value> MapFunction<TimespanValue<Resolution, Value>> for ToStringFunction
where
    Resolution: Send,
    Value: Into<String>,
{
    type Output = TimespanValue<Resolution, String>;
    fn function(inner_data: TimespanValue<Resolution, Value>) -> Result<Self::Output, UpdateError> {
        Ok(TimespanValue {
            timespan: inner_data.timespan,
            value: inner_data.value.into(),
        })
    }
}

/// Convert (numeric) values to strings
pub type MapToString<D> = Map<D, ToStringFunction>;
