use std::{fmt::Display, marker::PhantomData, str::FromStr};

use crate::{
    charts::db_interaction::types::DateValue, data_source::DataSource, DateValueString, Named,
    UpdateError,
};

use super::{SourceAdapter, SourceAdapterWrapper};

pub mod point;

pub trait ParseAdapter {
    type InnerSource: DataSource<Output = Vec<DateValueString>> + Named;
    type ParseInto: DateValue + Send;
}

/// Wrapper to convert type implementing [`ParseAdapter`] to another that implements [`DataSource`]
pub type ParseAdapterWrapper<T> = SourceAdapterWrapper<ParseAdapterLocalWrapper<T>>;

/// Wrapper to get type implementing "parent" trait. Use [`ParseAdapterWrapper`] to get [`DataSource`]
pub struct ParseAdapterLocalWrapper<T: ParseAdapter>(PhantomData<T>);

impl<T: ParseAdapter> SourceAdapter for ParseAdapterLocalWrapper<T>
where
    <T::ParseInto as DateValue>::Value: FromStr,
    <<T::ParseInto as DateValue>::Value as FromStr>::Err: Display,
{
    type InnerSource = T::InnerSource;
    type Output = Vec<T::ParseInto>;

    fn function(inner_data: Vec<DateValueString>) -> Result<Self::Output, UpdateError> {
        let parsed_data = inner_data
            .into_iter()
            .map(|p| {
                let (date, val_str) = p.into_parts();
                let val_parsed = val_str
                    .parse::<<T::ParseInto as DateValue>::Value>()
                    .map_err(|e| {
                        UpdateError::Internal(format!(
                            "failed to parse values in chart '{}': {e}",
                            T::InnerSource::NAME
                        ))
                    })?;
                Ok(T::ParseInto::from_parts(date, val_parsed))
            })
            .collect::<Result<Vec<T::ParseInto>, UpdateError>>()?;
        Ok(parsed_data)
    }
}
