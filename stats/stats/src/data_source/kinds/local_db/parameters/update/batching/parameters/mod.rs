use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use crate::{charts::db_interaction::write::insert_data_many, DateValueString, UpdateError};

use super::parameter_traits::{BatchSizeUpperBound, BatchStepBehaviour};

mod cumulative;
mod delta;

pub use cumulative::*;
pub use delta::*;

pub struct Batch30Days;

impl BatchSizeUpperBound for Batch30Days {
    fn batch_max_size() -> chrono::Duration {
        chrono::Duration::days(30)
    }
}

pub struct BatchMax;

impl BatchSizeUpperBound for BatchMax {
    fn batch_max_size() -> chrono::Duration {
        chrono::Duration::max_value()
    }
}

/// Pass the vector data from main dependency right into the database
pub struct PassVecStep;

impl BatchStepBehaviour<Vec<DateValueString>, ()> for PassVecStep {
    async fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        _update_time: DateTime<Utc>,
        min_blockscout_block: i64,
        _last_accurate_point: Option<DateValueString>,
        main_data: Vec<DateValueString>,
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
