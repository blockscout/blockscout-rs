//! `map` for a data source, i.e. applies a function to the output
//! of some other source.

use std::marker::PhantomData;

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};

use crate::{
    data_source::{kinds::AdapterDataSource, DataSource, UpdateContext},
    range::UniversalRange,
    ChartError,
};

mod divide;
mod parse;
mod strip_extension;
mod to_string;
mod unwrap_or;
mod wrapper;

pub use divide::MapDivide;
pub use parse::MapParseTo;
pub use strip_extension::StripExt;
pub use to_string::MapToString;
pub use unwrap_or::UnwrapOr;
pub use wrapper::StripWrapper;

/// Apply `F` to each value queried from data source `D`
pub struct Map<D, F>(PhantomData<(D, F)>)
where
    D: DataSource,
    F: MapFunction<D::Output>;

pub trait MapFunction<Input> {
    type Output: Send;
    fn function(inner_data: Input) -> Result<Self::Output, ChartError>;
}

impl<D, F> AdapterDataSource for Map<D, F>
where
    D: DataSource,
    F: MapFunction<D::Output>,
{
    type MainDependencies = D;
    type ResolutionDependencies = ();
    type Output = F::Output;

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
