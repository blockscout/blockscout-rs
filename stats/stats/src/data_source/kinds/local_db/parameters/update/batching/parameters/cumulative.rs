//! Batch step logic for [cumulative chart](`crate::data_source::kinds::local_db::DailyCumulativeLocalDbChartSource`)

use std::{fmt::Display, marker::PhantomData, ops::Add, str::FromStr};

use chrono::{DateTime, Utc};
use rust_decimal::prelude::Zero;
use sea_orm::DatabaseConnection;

use crate::{
    data_source::kinds::local_db::parameters::update::batching::parameter_traits::BatchStepBehaviour,
    types::{Timespan, TimespanValue},
    ChartError, ChartProperties,
};

use super::PassVecStep;

/// Add last value to all points received from main dependency
/// and store as usual.
///
/// Used in cumulative charts
pub struct AddLastValueStep<ChartProps>(PhantomData<ChartProps>);

impl<Resolution, Value, ChartProps>
    BatchStepBehaviour<Resolution, Vec<TimespanValue<Resolution, Value>>, ()>
    for AddLastValueStep<ChartProps>
where
    Resolution: Timespan + Clone + Send + Sync,
    Value: FromStr + ToString + Add + Zero + Clone + Send,
    <Value as FromStr>::Err: Display,
    ChartProps: ChartProperties,
{
    async fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: DateTime<Utc>,
        min_blockscout_block: i64,
        last_accurate_point: TimespanValue<Resolution, String>,
        main_data: Vec<TimespanValue<Resolution, Value>>,
        _resolution_data: (),
    ) -> Result<usize, ChartError> {
        let partial_sum = last_accurate_point.value.parse::<Value>().map_err(|e| {
            ChartError::Internal(format!(
                "failed to parse value in chart '{}': {e}",
                ChartProps::key()
            ))
        })?;
        let main_data = main_data
            .into_iter()
            .map(|tv| {
                let new_v = tv.value + partial_sum.clone();
                TimespanValue::<Resolution, String> {
                    timespan: tv.timespan,
                    value: new_v.to_string(),
                }
            })
            .collect();
        <PassVecStep as BatchStepBehaviour<
            Resolution,
            Vec<TimespanValue<Resolution, String>>,
            (),
        >>::batch_update_values_step_with(
            db,
            chart_id,
            update_time,
            min_blockscout_block,
            last_accurate_point,
            main_data,
            (),
        )
        .await
    }
}
