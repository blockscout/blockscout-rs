use std::marker::PhantomData;

use crate::{data_source::types::Get, types::TimespanValue, ChartError};

use super::{Map, MapFunction};

pub struct UnwrapOrFunction<DefaultValue: Get>(PhantomData<DefaultValue>);

impl<DefaultValue, Resolution, Value> MapFunction<Vec<TimespanValue<Resolution, Option<Value>>>>
    for UnwrapOrFunction<DefaultValue>
where
    Resolution: Send,
    Value: Send,
    DefaultValue: Get<Value = Value>,
{
    type Output = Vec<TimespanValue<Resolution, Value>>;
    fn function(
        inner_data: Vec<TimespanValue<Resolution, Option<Value>>>,
    ) -> Result<Self::Output, ChartError> {
        Ok(inner_data
            .into_iter()
            .map(|p| TimespanValue {
                timespan: p.timespan,
                value: p.value.unwrap_or_else(DefaultValue::get),
            })
            .collect())
    }
}

impl<DefaultValue, Resolution, Value> MapFunction<TimespanValue<Resolution, Option<Value>>>
    for UnwrapOrFunction<DefaultValue>
where
    Resolution: Send,
    Value: Send,
    DefaultValue: Get<Value = Value>,
{
    type Output = TimespanValue<Resolution, Value>;
    fn function(
        inner_data: TimespanValue<Resolution, Option<Value>>,
    ) -> Result<Self::Output, ChartError> {
        Ok(TimespanValue {
            timespan: inner_data.timespan,
            value: inner_data.value.unwrap_or_else(DefaultValue::get),
        })
    }
}

/// Returns the data from `D` or provided default value.
///
/// [`crate::gettable_const`] is useful for defining the default value
pub type UnwrapOr<D, T> = Map<D, UnwrapOrFunction<T>>;
