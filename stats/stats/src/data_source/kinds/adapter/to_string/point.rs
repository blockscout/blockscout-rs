use std::marker::PhantomData;

use crate::{
    data_source::{
        kinds::adapter::{SourceAdapter, SourceAdapterWrapper},
        DataSource,
    },
    DateValueString, UpdateError,
};

pub trait ToStringPointAdapter {
    type InnerSource: DataSource;
}

/// Wrapper to convert type implementing [`ToStringPointAdapter`] to another that implements [`DataSource`]
pub type ToStringPointAdapterWrapper<T> = SourceAdapterWrapper<ToStringPointAdapterLocalWrapper<T>>;

/// Wrapper to get type implementing "parent" trait. Use [`ToStringPointAdapterWrapper`] to get [`DataSource`]
pub struct ToStringPointAdapterLocalWrapper<T>(PhantomData<T>);

impl<T: ToStringPointAdapter> SourceAdapter for ToStringPointAdapterLocalWrapper<T>
where
    <T::InnerSource as DataSource>::Output: Into<DateValueString>,
{
    type InnerSource = T::InnerSource;
    type Output = DateValueString;

    fn function(
        inner_data: <Self::InnerSource as DataSource>::Output,
    ) -> Result<Self::Output, UpdateError> {
        Ok(inner_data.into())
    }
}
