//! Data source adapter.
//! Allows manipulating data read from the inner
//! data source.
//!
//! Kinda like `map` for a data source. I.e. applies
//! a function to the output.

use std::{fmt::Display, marker::PhantomData, ops::RangeInclusive, str::FromStr};

use blockscout_metrics_tools::AggregateTimer;
use chrono::Utc;
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    charts::db_interaction::types::DateValue,
    data_source::{DataSource, UpdateContext},
    DateValueString, Named, UpdateError,
};

pub trait SourceAdapter {
    type InnerSource: DataSource;
    type Output: Send;
    fn function(inner_data: <Self::InnerSource as DataSource>::Output) -> Self::Output;
}

pub struct SourceAdapterWrapper<T: SourceAdapter>(PhantomData<T>);

impl<T: SourceAdapter + Named> Named for SourceAdapterWrapper<T> {
    const NAME: &'static str = T::NAME;
}

impl<T: SourceAdapter> DataSource for SourceAdapterWrapper<T> {
    type PrimaryDependency = T::InnerSource;
    type SecondaryDependencies = ();
    type Output = T::Output;

    // Adapter by itself does not store anything
    const MUTEX_ID: Option<&'static str> = None;

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<RangeInclusive<DateTimeUtc>>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        let inner_data =
            <T::InnerSource as DataSource>::query_data(cx, range, remote_fetch_timer).await?;
        let transformed = T::function(inner_data);
        Ok(transformed)
    }
}
