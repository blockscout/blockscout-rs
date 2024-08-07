use std::{fmt::Display, marker::PhantomData, str::FromStr};

use crate::{
    data_source::kinds::data_manipulation::map::MapFunction, types::TimespanValue, UpdateError,
};

use super::Map;

pub struct ParseToFunction<Value>(PhantomData<Value>);

impl<Resolution, Value> MapFunction<Vec<TimespanValue<Resolution, String>>>
    for ParseToFunction<Value>
where
    Resolution: Send,
    Value: FromStr + Send,
    <Value as FromStr>::Err: Display,
{
    type Output = Vec<TimespanValue<Resolution, Value>>;

    fn function(
        inner_data: Vec<TimespanValue<Resolution, String>>,
    ) -> Result<Vec<TimespanValue<Resolution, Value>>, UpdateError> {
        inner_data
            .into_iter()
            .map(|p| {
                let val_parsed = p.value.parse::<Value>().map_err(|e| {
                    UpdateError::Internal(format!("failed to parse values of dependency: {e}"))
                })?;
                Ok(TimespanValue {
                    timespan: p.timespan,
                    value: val_parsed,
                })
            })
            .collect::<Result<Vec<_>, UpdateError>>()
    }
}

impl<Resolution, Value> MapFunction<TimespanValue<Resolution, String>> for ParseToFunction<Value>
where
    Resolution: Send,
    Value: FromStr + Send,
    <Value as FromStr>::Err: Display,
{
    type Output = TimespanValue<Resolution, Value>;

    fn function(
        inner_data: TimespanValue<Resolution, String>,
    ) -> Result<Self::Output, UpdateError> {
        let val_parsed = inner_data.value.parse::<Value>().map_err(|e| {
            UpdateError::Internal(format!("failed to parse values of dependency: {e}"))
        })?;
        Ok(TimespanValue {
            timespan: inner_data.timespan,
            value: val_parsed,
        })
    }
}

/// Parse string values to specified point type `P`
pub type MapParseTo<D, P> = Map<D, ParseToFunction<P>>;
