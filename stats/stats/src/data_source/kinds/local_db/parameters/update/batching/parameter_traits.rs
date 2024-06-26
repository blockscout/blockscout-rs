use std::future::Future;

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use crate::{DateValueString, UpdateError};

pub trait BatchSizeUpperBound {
    /// Upper bound of batch interval
    fn batch_max_size() -> chrono::Duration;
}

pub trait BatchStepBehaviour<MainInput, ResolutionInput>
where
    MainInput: Send,
    ResolutionInput: Send,
{
    /// Update chart with data from its dependencies.
    ///
    /// Returns how many records were found
    fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: DateTime<Utc>,
        min_blockscout_block: i64,
        last_accurate_point: Option<DateValueString>,
        main_data: MainInput,
        resolution_data: ResolutionInput,
    ) -> impl Future<Output = Result<usize, UpdateError>> + std::marker::Send;
}
