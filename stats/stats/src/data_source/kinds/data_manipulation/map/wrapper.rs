use sea_orm::TryGetable;

use crate::{
    data_source::{kinds::data_manipulation::map::MapFunction, types::WrappedValue},
    types::TimespanValue,
    ChartError,
};

use super::Map;

pub struct StripWrapperFunction;

impl<Resolution, Value> MapFunction<Vec<TimespanValue<Resolution, WrappedValue<Value>>>>
    for StripWrapperFunction
where
    Resolution: Send,
    Value: TryGetable + Send,
{
    type Output = Vec<TimespanValue<Resolution, Value>>;

    fn function(
        inner_data: Vec<TimespanValue<Resolution, WrappedValue<Value>>>,
    ) -> Result<Vec<TimespanValue<Resolution, Value>>, ChartError> {
        Ok(inner_data
            .into_iter()
            .map(|p| TimespanValue {
                timespan: p.timespan,
                value: p.value.into_inner(),
            })
            .collect::<Vec<_>>())
    }
}

impl<Resolution, Value> MapFunction<TimespanValue<Resolution, WrappedValue<Value>>>
    for StripWrapperFunction
where
    Resolution: Send,
    Value: TryGetable + Send,
{
    type Output = TimespanValue<Resolution, Value>;

    fn function(
        inner_data: TimespanValue<Resolution, WrappedValue<Value>>,
    ) -> Result<Self::Output, ChartError> {
        Ok(TimespanValue {
            timespan: inner_data.timespan,
            value: inner_data.value.into_inner(),
        })
    }
}

/// Remove [`WrappedValue`]
pub type StripWrapper<D> = Map<D, StripWrapperFunction>;
