//! Data source adapter.
//! Allows manipulating data read from the inner
//! data source.
//!
//! Kinda like `map` for a data source. I.e. applies
//! a function to the output.

use std::{marker::PhantomData, ops::RangeInclusive};

use blockscout_metrics_tools::AggregateTimer;
use chrono::Utc;
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    data_source::{DataSource, UpdateContext},
    UpdateError,
};

pub mod parse;
pub mod to_string;

pub trait MapFunction<Input> {
    type Output: Send;
    fn function(inner_data: Input) -> Result<Self::Output, UpdateError>;
}

pub struct Map<D, F>(PhantomData<(D, F)>)
where
    D: DataSource,
    F: MapFunction<D::Output>;

impl<D, F> DataSource for Map<D, F>
where
    D: DataSource,
    F: MapFunction<D::Output>,
{
    type MainDependencies = D;
    type ResolutionDependencies = ();
    type Output = F::Output;
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
        let inner_data = <D as DataSource>::query_data(cx, range, remote_fetch_timer).await?;
        let transformed = F::function(inner_data)?;
        Ok(transformed)
    }
}
