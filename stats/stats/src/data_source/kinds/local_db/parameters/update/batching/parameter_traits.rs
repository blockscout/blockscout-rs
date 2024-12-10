use std::future::Future;

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use crate::{
    types::{Timespan, TimespanValue},
    UpdateError,
};

pub trait BatchStepBehaviour<Resolution, MainInput>
where
    Resolution: Timespan,
    MainInput: Send,
{
    /// Update chart with data from its dependencies.
    ///
    /// Returns how many records were found
    fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: DateTime<Utc>,
        min_blockscout_block: i64,
        last_accurate_point: TimespanValue<Resolution, String>,
        main_data: MainInput,
    ) -> impl Future<Output = Result<usize, UpdateError>> + Send;
}
