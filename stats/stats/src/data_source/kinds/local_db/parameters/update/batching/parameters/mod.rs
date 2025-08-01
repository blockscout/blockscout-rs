use std::fmt::Debug;

use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::{ConnectionTrait, TransactionTrait};

use crate::{
    ChartError,
    charts::db_interaction::write::clear_all_chart_data,
    data_source::kinds::local_db::parameters::update::pass_vec,
    gettable_const,
    types::{
        Timespan, TimespanDuration, TimespanValue,
        timespans::{Month, Week, Year},
    },
};

use super::parameter_traits::BatchStepBehaviour;

mod cumulative;
#[cfg(any(feature = "test-utils", test))]
pub mod mock;

pub use cumulative::*;

gettable_const!(Batch30Days: TimespanDuration<NaiveDate> = TimespanDuration::from_days(30));
gettable_const!(BatchMaxDays: TimespanDuration<NaiveDate> = TimespanDuration::from_days(u64::MAX));
gettable_const!(Batch30Weeks: TimespanDuration<Week> = TimespanDuration::from_timespan_repeats(30));
gettable_const!(BatchMaxWeeks: TimespanDuration<Week> = TimespanDuration::from_timespan_repeats(u64::MAX));
gettable_const!(Batch36Months: TimespanDuration<Month> = TimespanDuration::from_timespan_repeats(36));
gettable_const!(Batch30Years: TimespanDuration<Year> = TimespanDuration::from_timespan_repeats(30));

/// Pass the vector data from main dependency right into the database
pub struct PassVecStep;

impl<Resolution> BatchStepBehaviour<Resolution, Vec<TimespanValue<Resolution, String>>, ()>
    for PassVecStep
where
    Resolution: Timespan + Clone + Send + Sync,
{
    async fn batch_update_values_step_with<C: ConnectionTrait + TransactionTrait>(
        db: &C,
        chart_id: i32,
        _update_time: DateTime<Utc>,
        min_indexer_block: i64,
        _last_accurate_point: TimespanValue<Resolution, String>,
        main_data: Vec<TimespanValue<Resolution, String>>,
        _resolution_data: (),
    ) -> Result<usize, ChartError> {
        pass_vec(db, chart_id, min_indexer_block, main_data).await
    }
}

/// Batch update "step" that clears all data for this chart
/// and inserts the newly queried data.
///
/// Since it removes all data each time, it makes sense to
/// use it only with max batch size.
pub struct ClearAllAndPassStep;

impl<Resolution> BatchStepBehaviour<Resolution, Vec<TimespanValue<Resolution, String>>, ()>
    for ClearAllAndPassStep
where
    Resolution: Timespan + Clone + Send + Sync + Debug,
{
    async fn batch_update_values_step_with<C: ConnectionTrait + TransactionTrait>(
        db: &C,
        chart_id: i32,
        _update_time: DateTime<Utc>,
        min_indexer_block: i64,
        _last_accurate_point: TimespanValue<Resolution, String>,
        main_data: Vec<TimespanValue<Resolution, String>>,
        _resolution_data: (),
    ) -> Result<usize, ChartError> {
        let db = db.begin().await.map_err(ChartError::StatsDB)?;
        clear_all_chart_data(&db, chart_id)
            .await
            .map_err(ChartError::StatsDB)?;
        let result = PassVecStep::batch_update_values_step_with(
            &db,
            chart_id,
            _update_time,
            min_indexer_block,
            _last_accurate_point,
            main_data,
            _resolution_data,
        )
        .await?;
        db.commit().await.map_err(ChartError::StatsDB)?;
        Ok(result)
    }
}
