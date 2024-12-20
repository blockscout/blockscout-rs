//! `map` for a data source, i.e. applies a function to the output
//! of some other source.

use std::marker::PhantomData;

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    data_source::{DataSource, UpdateContext},
    range::UniversalRange,
    ChartError,
};

mod parse;
mod strip_extension;
mod to_string;
mod unwrap_or;

pub use parse::MapParseTo;
pub use strip_extension::StripExt;
pub use to_string::MapToString;
pub use unwrap_or::UnwrapOr;

/// Apply `F` to each value queried from data source `D`
pub struct Map<D, F>(PhantomData<(D, F)>)
where
    D: DataSource,
    F: MapFunction<D::Output>;

pub trait MapFunction<Input> {
    type Output: Send;
    fn function(inner_data: Input) -> Result<Self::Output, ChartError>;
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

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), ChartError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        let inner_data =
            <D as DataSource>::query_data(cx, range, dependency_data_fetch_timer).await?;
        F::function(inner_data)
    }
}
