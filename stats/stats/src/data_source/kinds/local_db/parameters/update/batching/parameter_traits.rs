use std::future::Future;

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, TransactionTrait};

use crate::{
    types::{Timespan, TimespanValue},
    ChartError,
};

pub trait BatchStepBehaviour<Resolution, MainInput, ResolutionInput>
where
    Resolution: Timespan,
    MainInput: Send,
    ResolutionInput: Send,
{
    /// Update chart with data from its dependencies.
    ///
    /// Returns how many records were found
    fn batch_update_values_step_with<C: ConnectionTrait + TransactionTrait>(
        db: &C,
        chart_id: i32,
        update_time: DateTime<Utc>,
        min_blockscout_block: i64,
        last_accurate_point: TimespanValue<Resolution, String>,
        main_data: MainInput,
        resolution_data: ResolutionInput,
    ) -> impl Future<Output = Result<usize, ChartError>> + std::marker::Send;
}
