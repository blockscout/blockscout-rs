use std::marker::PhantomData;

use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::{ConnectionTrait, TransactionTrait};

use crate::{
    data_source::kinds::local_db::parameters::update::batching::parameter_traits::BatchStepBehaviour,
    tests::recorder::Recorder, types::timespans::DateValue, ChartError,
};

use super::PassVecStep;

#[derive(Debug, Clone)]
pub struct StepInput<MI, RI> {
    pub chart_id: i32,
    pub update_time: DateTime<Utc>,
    pub min_blockscout_block: i64,
    pub last_accurate_point: DateValue<String>,
    pub main_data: MI,
    pub resolution_data: RI,
}

/// Allows to inspect values passed to each step call;
/// does nothind regarding side effects
#[derive(Default)]
pub struct RecordingPassStep<StepsRecorder>(PhantomData<StepsRecorder>)
where
    StepsRecorder: Recorder;

impl<StepsRecorder> BatchStepBehaviour<NaiveDate, Vec<DateValue<String>>, ()>
    for RecordingPassStep<StepsRecorder>
where
    StepsRecorder: Recorder<Data = StepInput<Vec<DateValue<String>>, ()>>,
{
    async fn batch_update_values_step_with<C>(
        db: &C,
        chart_id: i32,
        update_time: DateTime<Utc>,
        min_blockscout_block: i64,
        last_accurate_point: DateValue<String>,
        main_data: Vec<DateValue<String>>,
        resolution_data: (),
    ) -> Result<usize, ChartError>
    where
        C: ConnectionTrait + TransactionTrait,
    {
        StepsRecorder::record(StepInput {
            chart_id,
            update_time,
            min_blockscout_block,
            last_accurate_point: last_accurate_point.clone(),
            main_data: main_data.clone(),
            resolution_data,
        })
        .await;
        PassVecStep::batch_update_values_step_with(
            db,
            chart_id,
            update_time,
            min_blockscout_block,
            last_accurate_point,
            main_data,
            resolution_data,
        )
        .await
    }
}
