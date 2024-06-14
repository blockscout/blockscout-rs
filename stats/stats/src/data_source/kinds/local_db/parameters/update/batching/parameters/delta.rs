//! The opposite of [cumulative chart](crate::data_source::kinds::local_db::CumulativeLocalDbChartSource).
//!
//! I.e. chart "New accounts" is a delta
//! of  "Total accounts".

use std::{fmt::Display, marker::PhantomData, ops::SubAssign, str::FromStr};

use chrono::{DateTime, Utc};
use rust_decimal::prelude::Zero;
use sea_orm::DatabaseConnection;

use crate::{
    charts::chart::Point,
    data_processing::deltas,
    data_source::kinds::local_db::parameters::update::batching::{
        parameter_traits::BatchStepBehaviour, parameters::PassVecStep,
    },
    ChartProperties, DateValueString, UpdateError,
};

/// Add last value to all points received from main dependency
/// and store as usual.
///
/// Used in cumulative charts, for example
pub struct DeltaValuesStep<ChartProps>(PhantomData<ChartProps>);

impl<DV, ChartProps> BatchStepBehaviour<Vec<DV>, ()> for DeltaValuesStep<ChartProps>
where
    DV: Point + Default,
    DV::Value: FromStr + SubAssign + Zero + Clone,
    <DV::Value as FromStr>::Err: Display,
    ChartProps: ChartProperties,
{
    async fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: DateTime<Utc>,
        min_blockscout_block: i64,
        last_accurate_point: Option<DateValueString>,
        cum_data: Vec<DV>,
        _resolution_data: (),
    ) -> Result<usize, crate::UpdateError> {
        let prev_value = last_accurate_point
            .map(|p| {
                p.value.parse::<DV::Value>().map_err(|e| {
                    UpdateError::Internal(format!(
                        "failed to parse value in chart '{}': {e}",
                        ChartProps::NAME
                    ))
                })
            })
            .transpose()?;
        let prev_value = prev_value.unwrap_or(DV::Value::zero());
        let data = deltas::<DV>(cum_data, prev_value)?;
        let string_data: Vec<DateValueString> = data.into_iter().map(|p| p.into()).collect();

        PassVecStep::batch_update_values_step_with(
            db,
            chart_id,
            update_time,
            min_blockscout_block,
            None, // ignored anyway
            string_data,
            (),
        )
        .await
    }
}
