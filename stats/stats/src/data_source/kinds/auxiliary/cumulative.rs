use std::{marker::PhantomData, ops::AddAssign};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::Zero;
use sea_orm::DatabaseConnection;

use crate::{
    data_processing::cumsum,
    data_source::{DataSource, UpdateContext},
    range::UniversalRange,
    types::TimespanValue,
    ChartError,
};

/// Auxiliary source for cumulative chart.
///
/// Does not really make sense without using previous value, so
/// use [`super::super::local_db::DailyCumulativeLocalDbChartSource`] instead.
pub struct PartialCumulative<Delta: DataSource>(PhantomData<Delta>);

impl<Delta: DataSource> PartialCumulative<Delta> {}

impl<Delta, Resolution, Value> DataSource for PartialCumulative<Delta>
where
    Resolution: Send,
    Value: AddAssign + Zero + Clone + Send,
    TimespanValue<Resolution, Value>: Default,
    Delta: DataSource<Output = Vec<TimespanValue<Resolution, Value>>>,
{
    type MainDependencies = Delta;
    type ResolutionDependencies = ();
    type Output = Vec<TimespanValue<Resolution, Value>>;
    fn mutex_id() -> Option<String> {
        None
    }

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &DateTime<Utc>,
    ) -> Result<(), sea_orm::DbErr> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), ChartError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn set_next_update_from_itself(
        _db: &DatabaseConnection,
        _update_from: chrono::NaiveDate,
    ) -> Result<(), ChartError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        let delta_data = Delta::query_data(cx, range, dependency_data_fetch_timer).await?;
        let data = cumsum::<Resolution, Value>(delta_data, Value::zero())?;
        Ok(data)
    }
}
