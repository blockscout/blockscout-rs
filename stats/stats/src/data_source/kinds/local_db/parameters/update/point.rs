use std::marker::PhantomData;

use blockscout_metrics_tools::AggregateTimer;

use crate::{
    charts::db_interaction::write::insert_data_many,
    data_source::{kinds::local_db::UpdateBehaviour, DataSource, UpdateContext},
    types::{Timespan, TimespanValue},
    UpdateError,
};

/// Store output of the `MainDep` right in the local db
pub struct PassPoint<MainDep>(PhantomData<MainDep>);

impl<MainDep, Resolution> UpdateBehaviour<MainDep, (), Resolution> for PassPoint<MainDep>
where
    MainDep: DataSource<Output = TimespanValue<Resolution, String>>,
    Resolution: Timespan + Clone + Send,
{
    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        _last_accurate_point: Option<TimespanValue<Resolution, String>>,
        min_blockscout_block: i64,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        // range doesn't make sense there; thus is not used
        let data = MainDep::query_data(cx, None, remote_fetch_timer).await?;
        let value = data.active_model(chart_id, Some(min_blockscout_block));
        insert_data_many(cx.db, vec![value])
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}
