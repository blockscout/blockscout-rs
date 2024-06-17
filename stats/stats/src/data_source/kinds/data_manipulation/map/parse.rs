use std::{fmt::Display, marker::PhantomData, str::FromStr};

use crate::{
    charts::db_interaction::types::DateValue,
    data_source::kinds::data_manipulation::map::MapFunction, DateValueString, UpdateError,
};

use super::Map;

pub struct ParseVecTo<D: DateValue>(PhantomData<D>);

impl<D> MapFunction<Vec<DateValueString>> for ParseVecTo<D>
where
    D: DateValue + Send,
    D::Value: FromStr,
    <D::Value as FromStr>::Err: Display,
{
    type Output = Vec<D>;

    fn function(inner_data: Vec<DateValueString>) -> Result<Self::Output, UpdateError> {
        let parsed_data = inner_data
            .into_iter()
            .map(|p| {
                let (date, val_str) = p.into_parts();
                let val_parsed = val_str.parse::<D::Value>().map_err(|e| {
                    UpdateError::Internal(format!("failed to parse values of dependency: {e}"))
                })?;
                Ok(D::from_parts(date, val_parsed))
            })
            .collect::<Result<Vec<D>, UpdateError>>()?;
        Ok(parsed_data)
    }
}

pub struct ParsePointTo<D: DateValue>(PhantomData<D>);

impl<D> MapFunction<DateValueString> for ParsePointTo<D>
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

pub struct ParseToFunction<D: DateValue>(PhantomData<D>);

impl<D> MapFunction<Vec<DateValueString>> for ParseToFunction<D>
where
    D: DateValue + Send,
    D::Value: FromStr,
    <D::Value as FromStr>::Err: Display,
{
    type Output = Vec<D>;

    fn function(inner_data: Vec<DateValueString>) -> Result<Vec<D>, UpdateError> {
        let parsed_data = inner_data
            .into_iter()
            .map(|p| {
                let (date, val_str) = p.into_parts();
                let val_parsed = val_str.parse::<D::Value>().map_err(|e| {
                    UpdateError::Internal(format!("failed to parse values of dependency: {e}"))
                })?;
                Ok(D::from_parts(date, val_parsed))
            })
            .collect::<Result<Vec<D>, UpdateError>>()?;
        Ok(parsed_data)
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
