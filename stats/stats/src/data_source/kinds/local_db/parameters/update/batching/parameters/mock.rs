use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use crate::{
    data_source::kinds::local_db::parameters::update::batching::parameter_traits::BatchStepBehaviour,
    tests::recorder::Recorder, DateValueString, UpdateError,
};

use super::PassVecStep;

#[derive(Debug, Clone)]
pub struct StepInput<MI, RI> {
    pub chart_id: i32,
    pub update_time: DateTime<Utc>,
    pub min_blockscout_block: i64,
    pub last_accurate_point: Option<DateValueString>,
    pub main_data: MI,
    pub resolution_data: RI,
}

/// Allows to inspect values passed to each step call;
/// does nothind regarding side effects
#[derive(Default)]
pub struct RecordingPassStep<StepsRecorder>(PhantomData<StepsRecorder>)
where
    StepsRecorder: Recorder;

impl<StepsRecorder> BatchStepBehaviour<Vec<DateValueString>, ()>
    for RecordingPassStep<StepsRecorder>
where
    StepsRecorder: Recorder<Data = StepInput<Vec<DateValueString>, ()>>,
{
    async fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: DateTime<Utc>,
        min_blockscout_block: i64,
        last_accurate_point: Option<DateValueString>,
        main_data: Vec<DateValueString>,
        resolution_data: (),
    ) -> Result<usize, UpdateError> {
        StepsRecorder::record(StepInput {
            chart_id,
            update_time,
            min_blockscout_block,
            last_accurate_point: last_accurate_point.clone(),
            main_data: main_data.clone(),
            resolution_data,
        });
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
