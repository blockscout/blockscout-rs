//! Chart that accumulates values of another chart.
//!
//! For example, chart "Total accounts" is a cumulative
//! of "New accounts".
//!
//! So, if the values of `NewItemsChart` are [1, 2, 3, 4], then
//! cumulative chart will produce [1, 3, 6, 10].
//!
//! The opposite of this chart is [delta chart](super::delta).

use std::{fmt::Display, marker::PhantomData, ops::Add, str::FromStr};

use chrono::{DateTime, Utc};
use rust_decimal::prelude::Zero;
use sea_orm::DatabaseConnection;

use crate::{
    charts::db_interaction::types::DateValue,
    data_source::kinds::local_db::parameters::update::batching::parameter_traits::BatchStepBehaviour,
    ChartProperties, DateValueString, UpdateError,
};

use super::PassVecStep;

/// Add last value to all points received from main dependency
/// and store as usual.
///
/// Used in cumulative charts, for example
pub struct AddLastValueStep<ChartProps>(PhantomData<ChartProps>);

impl<DV, ChartProps> BatchStepBehaviour<Vec<DV>, ()> for AddLastValueStep<ChartProps>
where
    DV: DateValue + Send,
    DV::Value: FromStr + ToString + Add + Zero + Clone + Send,
    <DV::Value as FromStr>::Err: Display,
    ChartProps: ChartProperties,
{
    async fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: DateTime<Utc>,
        min_blockscout_block: i64,
        last_accurate_point: Option<DateValueString>,
        main_data: Vec<DV>,
        _resolution_data: (),
    ) -> Result<usize, UpdateError> {
        let partial_sum = last_accurate_point
            .map(|p| {
                p.value.parse::<DV::Value>().map_err(|e| {
                    UpdateError::Internal(format!(
                        "failed to parse value in chart '{}': {e}",
                        ChartProps::NAME
                    ))
                })
            })
            .transpose()?;
        let partial_sum = partial_sum.unwrap_or(DV::Value::zero());
        let main_data = main_data
            .into_iter()
            .map(|dv| {
                let (d, v) = dv.into_parts();
                let new_v = v + partial_sum.clone();
                DateValueString::from_parts(d, new_v.to_string())
            })
            .collect();
        PassVecStep::batch_update_values_step_with(
            db,
            chart_id,
            update_time,
            min_blockscout_block,
            None, // ignored anyway
            main_data,
            (),
        )
        .await
    }
}
