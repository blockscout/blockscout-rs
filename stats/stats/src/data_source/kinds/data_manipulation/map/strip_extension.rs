use crate::{
    types::{ExtendedTimespanValue, TimespanValue},
    ChartError,
};

use super::{Map, MapFunction};

/// Remove `Extended` part from `ExtendedTimespanValue`.
/// Used because it's easier to impl and use other data source modifiers
/// this way.
pub struct StripExtensionFunction;

impl<R, V> MapFunction<Vec<ExtendedTimespanValue<R, V>>> for StripExtensionFunction
where
    R: Send,
    V: Send,
{
    type Output = Vec<TimespanValue<R, V>>;
    fn function(inner_data: Vec<ExtendedTimespanValue<R, V>>) -> Result<Self::Output, ChartError> {
        Ok(inner_data.into_iter().map(|p| p.into()).collect())
    }
}

impl<R, V> MapFunction<ExtendedTimespanValue<R, V>> for StripExtensionFunction
where
    R: Send,
    V: Send,
{
    type Output = TimespanValue<R, V>;
    fn function(inner_data: ExtendedTimespanValue<R, V>) -> Result<Self::Output, ChartError> {
        Ok(inner_data.into())
    }
}

/// Remove `Extended` part from `ExtendedTimespanValue`(-s)
pub type StripExt<D> = Map<D, StripExtensionFunction>;
