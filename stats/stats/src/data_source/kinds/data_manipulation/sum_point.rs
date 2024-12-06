//! Sum data source.
//!
//! Sums all points from the other (vector) source.

use std::{marker::PhantomData, ops::AddAssign};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::Zero;
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    data_processing::sum,
    data_source::{source::DataSource, UpdateContext},
    range::UniversalRange,
    types::{Timespan, TimespanValue},
    ChartError,
};

/// Sum all dependency's data.
///
/// Missing points in dependency's output are expected to mean zero value
/// (==`MissingDatePolicy::FillZero`).
/// [see "Dependency requirements" here](crate::data_source::kinds)
///
/// See [module-level documentation](self) for more details.
pub struct Sum<DS>(PhantomData<DS>)
where
    DS: DataSource;

impl<DS, Resolution, Value> DataSource for Sum<DS>
where
    Resolution: Timespan + Ord + Clone + Send,
    Value: AddAssign + Clone + Zero + Send,
    DS: DataSource<Output = Vec<TimespanValue<Resolution, Value>>>,
{
    type MainDependencies = DS;
    type ResolutionDependencies = ();
    type Output = TimespanValue<Resolution, Value>;
    fn mutex_id() -> Option<String> {
        None
    }

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &DateTime<Utc>,
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
        _range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        // it's possible to not request full data range and use last accurate point;
        // can be updated to work similarly to cumulative
        let full_data =
            DS::query_data(cx, UniversalRange::full(), dependency_data_fetch_timer).await?;
        tracing::debug!(points_len = full_data.len(), "calculating sum");
        let zero = Value::zero();
        sum::<Resolution, Value>(&full_data, zero)
    }
}
