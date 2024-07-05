use std::{fmt::Display, marker::PhantomData, str::FromStr};

use crate::{
    charts::types::DateValue, data_source::kinds::data_manipulation::map::MapFunction,
    types::TimespanValue, DateValueString, UpdateError,
};

use super::Map;

pub struct ParseToFunction<D: DateValue>(PhantomData<D>);

impl<D> MapFunction<Vec<DateValueString>> for ParseToFunction<D>
where
    D: DateValue + Send,
    D::Value: FromStr,
    <D::Value as FromStr>::Err: Display,
{
    type Output = Vec<D>;

    fn function(inner_data: Vec<DateValueString>) -> Result<Vec<D>, UpdateError> {
        inner_data
            .into_iter()
            .map(|p| {
                let (date, val_str) = p.into_parts();
                let val_parsed = val_str.parse::<D::Value>().map_err(|e| {
                    UpdateError::Internal(format!("failed to parse values of dependency: {e}"))
                })?;
                Ok(D::from_parts(date, val_parsed))
            })
            .collect::<Result<Vec<D>, UpdateError>>()
    }
}

impl<D> MapFunction<DateValueString> for ParseToFunction<D>
where
    D: DateValue + Send,
    D::Value: FromStr,
    <D::Value as FromStr>::Err: Display,
{
    type Output = D;

    fn function(inner_data: DateValueString) -> Result<Self::Output, UpdateError> {
        let (date, val_str) = inner_data.into_parts();
        let val_parsed = val_str.parse::<D::Value>().map_err(|e| {
            UpdateError::Internal(format!("failed to parse values of dependency: {e}"))
        })?;
        Ok(D::from_parts(date, val_parsed))
    }
}

/// Parse string values to specified point type `P`
pub type MapParseTo<D, P> = Map<D, ParseToFunction<P>>;
