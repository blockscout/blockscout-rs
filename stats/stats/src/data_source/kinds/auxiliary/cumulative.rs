use std::{marker::PhantomData, ops::AddAssign};

use rust_decimal::prelude::Zero;

use crate::{
    charts::db_interaction::types::DateValue, data_processing::cumsum, data_source::DataSource,
};

/// Auxiliary source for cumulative chart.
///
/// Does not really make sense without using previous value, so
/// use [`super::super::local_db::CumulativeLocalDbChartSource`] instead.
pub struct PartialCumulative<Delta: DataSource>(PhantomData<Delta>);

impl<Delta: DataSource> PartialCumulative<Delta> {}

impl<Delta, DV> DataSource for PartialCumulative<Delta>
where
    Delta: DataSource<Output = Vec<DV>>,
    DV: DateValue + Send + Default,
    DV::Value: AddAssign + Zero + Clone,
{
    type MainDependencies = Delta;
    type ResolutionDependencies = ();
    type Output = Vec<DV>;
    const MUTEX_ID: Option<&'static str> = None;

    async fn init_itself(
        _db: &sea_orm::DatabaseConnection,
        _init_time: &chrono::DateTime<chrono::Utc>,
    ) -> Result<(), sea_orm::DbErr> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn update_itself(
        _cx: &crate::data_source::UpdateContext<'_>,
    ) -> Result<(), crate::UpdateError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn query_data(
        cx: &crate::data_source::UpdateContext<'_>,
        range: Option<std::ops::RangeInclusive<sea_orm::prelude::DateTimeUtc>>,
        dependency_data_fetch_timer: &mut blockscout_metrics_tools::AggregateTimer,
    ) -> Result<Self::Output, crate::UpdateError> {
        let delta_data = Delta::query_data(cx, range, dependency_data_fetch_timer).await?;
        let data = cumsum::<DV>(delta_data, DV::Value::zero())?;
        Ok(data)
    }
}
