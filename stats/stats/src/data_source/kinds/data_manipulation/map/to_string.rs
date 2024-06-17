use crate::{DateValueString, UpdateError};

use super::{Map, MapFunction};

pub struct ToStringFunction;

impl<D: Into<DateValueString>> MapFunction<Vec<D>> for ToStringFunction {
    type Output = Vec<DateValueString>;
    fn function(inner_data: Vec<D>) -> Result<Self::Output, UpdateError> {
        Ok(inner_data.into_iter().map(|p| p.into()).collect())
    }
}

impl<D: Into<DateValueString>> MapFunction<D> for ToStringFunction {
    type Output = DateValueString;
    fn function(inner_data: D) -> Result<Self::Output, UpdateError> {
        Ok(inner_data.into())
    }
}

/// Convert (numeric) values to strings
pub type MapToString<D> = Map<D, ToStringFunction>;
