use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use crate::{
    charts::db_interaction::write::insert_data_many,
    gettable_const,
    types::{Timespan, TimespanValue},
    UpdateError,
};

use super::parameter_traits::BatchStepBehaviour;

mod cumulative;
#[cfg(any(feature = "test-utils", test))]
pub mod mock;

pub use cumulative::*;

gettable_const!(Batch30Days: chrono::Duration = chrono::Duration::days(30));
gettable_const!(BatchMax: chrono::Duration = chrono::Duration::max_value());

/// Pass the vector data from main dependency right into the database
pub struct PassVecStep;

impl<Resolution> BatchStepBehaviour<Resolution, Vec<TimespanValue<Resolution, String>>, ()>
    for PassVecStep
where
    Resolution: Timespan + Clone + Send + Sync,
{
    async fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        _update_time: DateTime<Utc>,
        min_blockscout_block: i64,
        _last_accurate_point: TimespanValue<Resolution, String>,
        main_data: Vec<TimespanValue<Resolution, String>>,
        _resolution_data: (),
    ) -> Result<usize, UpdateError> {
        let found = main_data.len();
        // note: right away cloning another chart will not result in exact copy,
        // because if the other chart is `FillPrevious`, then omitted starting point
        // within the range is set to the last known before the range
        // i.e. some duplicate points might get added.
        //
        // however, it should not be a problem, since semantics remain the same +
        // cloning already stored chart is counter-productive/not effective.
        let values = main_data
            .into_iter()
            .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(found)
    }
}
