//! `map` for a data source, i.e. applies a function to the output
//! of some other source.

use std::{marker::PhantomData, ops::Range};

use blockscout_metrics_tools::AggregateTimer;
use chrono::Utc;
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    data_source::{DataSource, UpdateContext},
    UpdateError,
};

mod parse;
mod to_string;

pub use parse::MapParseTo;
pub use to_string::MapToString;

/// Apply `F` to each value queried from data source `D`
pub struct Map<D, F>(PhantomData<(D, F)>)
where
    D: DataSource,
    F: MapFunction<D::Output>;

pub trait MapFunction<Input> {
    type Output: Send;
    fn function(inner_data: Input) -> Result<Self::Output, UpdateError>;
}

impl<D, F> DataSource for Map<D, F>
where
    D: DataSource,
    F: MapFunction<D::Output>,
{
    type MainDependencies = D;
    type ResolutionDependencies = ();
    type Output = F::Output;
    fn mutex_id() -> Option<String> {
                None
            }

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
        range: Option<Range<DateTimeUtc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        let inner_data =
            <D as DataSource>::query_data(cx, range, dependency_data_fetch_timer).await?;
        F::function(inner_data)
    }
}
