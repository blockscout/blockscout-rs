use std::{fmt::Display, marker::PhantomData, str::FromStr};

use crate::{
    charts::db_interaction::types::DateValue,
    data_source::{
        kinds::adapter::{SourceAdapter, SourceAdapterWrapper},
        DataSource,
    },
    DateValueString, Named, UpdateError,
};

pub trait ParsePointAdapter {
    type InnerSource: DataSource<Output = DateValueString> + Named;
    type ParseInto: DateValue + Send;
}

/// Wrapper to convert type implementing [`ParsePointAdapter`] to another that implements [`DataSource`]
pub type ParsePointAdapterWrapper<T> = SourceAdapterWrapper<ParsePointAdapterLocalWrapper<T>>;

/// Wrapper to get type implementing "parent" trait. Use [`ParsePointAdapterWrapper`] to get [`DataSource`]
pub struct ParsePointAdapterLocalWrapper<T: ParsePointAdapter>(PhantomData<T>);

impl<T: ParsePointAdapter> SourceAdapter for ParsePointAdapterLocalWrapper<T>
where
    <T::ParseInto as DateValue>::Value: FromStr,
    <<T::ParseInto as DateValue>::Value as FromStr>::Err: Display,
{
    type InnerSource = T::InnerSource;
    type Output = T::ParseInto;

    fn function(inner_data: DateValueString) -> Result<Self::Output, UpdateError> {
        let (date, val_str) = inner_data.into_parts();
        let val_parsed = val_str
            .parse::<<T::ParseInto as DateValue>::Value>()
            .map_err(|e| {
                UpdateError::Internal(format!(
                    "failed to parse values in chart '{}': {e}",
                    T::InnerSource::NAME
                ))
            })?;
        Ok(T::ParseInto::from_parts(date, val_parsed))
    }
}
