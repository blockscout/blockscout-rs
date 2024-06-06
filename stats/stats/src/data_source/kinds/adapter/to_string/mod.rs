use std::marker::PhantomData;

use crate::{data_source::DataSource, DateValueString, UpdateError};

use super::{SourceAdapter, SourceAdapterWrapper};

pub mod point;

pub trait ToStringAdapter {
    type InnerSource: DataSource<Output = Vec<Self::ConvertFrom>>;
    // only need because couldn't figure out how to "extract" generic
    // T from `Vec` and place bounds on it
    /// Type of elements in the output of [`InnerSource`](ToStringAdapter::InnerSource)
    type ConvertFrom: Into<DateValueString>;
}

/// Wrapper to convert type implementing [`ToStringAdapter`] to another that implements [`DataSource`]
pub type ToStringAdapterWrapper<T> = SourceAdapterWrapper<ToStringAdapterLocalWrapper<T>>;

/// Wrapper to get type implementing "parent" trait. Use [`ToStringAdapterWrapper`] to get [`DataSource`]
pub struct ToStringAdapterLocalWrapper<T>(PhantomData<T>);

impl<T: ToStringAdapter> SourceAdapter for ToStringAdapterLocalWrapper<T> {
    type InnerSource = T::InnerSource;
    type Output = Vec<DateValueString>;

    fn function(
        inner_data: <Self::InnerSource as DataSource>::Output,
    ) -> Result<Self::Output, UpdateError> {
        Ok(inner_data.into_iter().map(|p| p.into()).collect())
    }
}
