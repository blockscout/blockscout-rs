//! Batching for update process.
//!
//! Update for some period P can be done only with dependencies'
//! data for the same exact period P.

use std::{marker::PhantomData, ops::Range, time::Instant};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Days, Utc};
use parameter_traits::BatchStepBehaviour;
use sea_orm::TransactionTrait;

use crate::{
    charts::db_interaction::read::get_min_date_blockscout,
    data_source::{
        kinds::local_db::{parameter_traits::QueryBehaviour, UpdateBehaviour},
        source::DataSource,
        types::Get,
        UpdateContext,
    },
    utils::day_start,
    ChartProperties, DateValueString, UpdateError, ZeroDateValue,
};

pub mod parameter_traits;
pub mod parameters;

pub struct BatchUpdate<MainDep, ResolutionDep, BatchStep, BatchSizeUpperBound, Query, ChartProps>(
    PhantomData<(
        MainDep,
        ResolutionDep,
        BatchStep,
        BatchSizeUpperBound,
        Query,
        ChartProps,
    )>,
)
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    BatchStep: BatchStepBehaviour<MainDep::Output, ResolutionDep::Output>,
    BatchSizeUpperBound: Get<chrono::Duration>,
    Query: QueryBehaviour<Output = Vec<DateValueString>>,
    ChartProps: ChartProperties;

impl<MainDep, ResolutionDep, BatchStep, BatchSizeUpperBound, Query, ChartProps>
    UpdateBehaviour<MainDep, ResolutionDep>
    for BatchUpdate<MainDep, ResolutionDep, BatchStep, BatchSizeUpperBound, Query, ChartProps>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    BatchStep: BatchStepBehaviour<MainDep::Output, ResolutionDep::Output>,
    BatchSizeUpperBound: Get<chrono::Duration>,
    Query: QueryBehaviour<Output = Vec<DateValueString>>,
    ChartProps: ChartProperties,
{
    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        last_accurate_point: Option<DateValueString>,
        min_blockscout_block: i64,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let now = cx.time;
        let txn = cx
            .blockscout
            .begin()
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let update_from_date = last_accurate_point
            .clone()
            .and_then(|p| p.date.checked_add_days(Days::new(1)));
        let first_date = match update_from_date {
            Some(d) => d,
            None => get_min_date_blockscout(&txn)
                .await
                .map(|time| time.date())
                .map_err(UpdateError::BlockscoutDB)?,
        };
        let first_date_time = day_start(first_date);

        let steps = generate_date_time_ranges(first_date_time, now, BatchSizeUpperBound::get());
        let n = steps.len();

        for (i, range) in steps.into_iter().enumerate() {
            let previous_step_last_point =
                get_previous_step_last_point::<Query>(cx, range.clone()).await?;
            tracing::info!(from =? range.start, to =? range.end, previous_step_last_point =? previous_step_last_point, "run {}/{} step of batch update", i + 1, n);
            let now = Instant::now();
            let found = batch_update_values_step::<MainDep, ResolutionDep, BatchStep>(
                cx,
                chart_id,
                min_blockscout_block,
                previous_step_last_point,
                range,
                dependency_data_fetch_timer,
            )
            .await?;
            let elapsed: std::time::Duration = now.elapsed();
            tracing::info!(found =? found, elapsed =? elapsed, "{}/{} step of batch done", i + 1, n);
        }
        Ok(())
    }
}

/// Errors if no point is found
async fn get_previous_step_last_point<Query>(
    cx: &UpdateContext<'_>,
    this_step_range: Range<DateTime<Utc>>,
) -> Result<DateValueString, UpdateError>
where
    Query: QueryBehaviour<Output = Vec<DateValueString>>,
{
    // for now the only resolution is 1 day
    let resolution = chrono::Duration::days(1);
    let last_point_range = (this_step_range.start - resolution)..this_step_range.start;
    // must be within the same day
    debug_assert_eq!(
        day_start(last_point_range.start.date_naive()),
        day_start((last_point_range.end - chrono::Duration::nanoseconds(1)).date_naive())
    );
    let last_point_range_values = Query::query_data(cx, Some(last_point_range.clone())).await?;
    let previous_step_last_point = last_point_range_values
        .last()
        .cloned()
        // `None` means
        // - if `MissingDatePolicy` is `FillZero` = the value is 0
        // - if `MissingDatePolicy` is `FillPrevious` = no previous value was found at this moment in time = value at the point is 0
        .unwrap_or(DateValueString::with_zero_value(
            // we store/operate with dates only for now
            last_point_range.start.date_naive(),
        ));
    if last_point_range_values.len() > 1 {
        // return error because it's likely that date in `previous_step_last_point` is incorrect
        return Err(UpdateError::Internal("Retrieved 2 points from previous step; probably an issue with range construction and handling".to_owned()));
    }
    Ok(previous_step_last_point)
}

/// Returns how many records were found
async fn batch_update_values_step<MainDep, ResolutionDep, BatchStep>(
    cx: &UpdateContext<'_>,
    chart_id: i32,
    min_blockscout_block: i64,
    last_accurate_point: DateValueString,
    range: Range<DateTime<Utc>>,
    dependency_data_fetch_timer: &mut AggregateTimer,
) -> Result<usize, UpdateError>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    BatchStep: BatchStepBehaviour<MainDep::Output, ResolutionDep::Output>,
{
    let main_data =
        MainDep::query_data(cx, Some(range.clone()), dependency_data_fetch_timer).await?;
    let resolution_data =
        ResolutionDep::query_data(cx, Some(range), dependency_data_fetch_timer).await?;
    let found = BatchStep::batch_update_values_step_with(
        cx.db,
        chart_id,
        cx.time,
        min_blockscout_block,
        last_accurate_point,
        main_data,
        resolution_data,
    )
    .await?;
    Ok(found)
}

/// Split the range [`start`, `end`) into multiple
/// with maximum length `step`
fn generate_date_time_ranges(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    max_step: chrono::Duration,
) -> Vec<Range<DateTime<Utc>>> {
    debug_assert!(
        max_step > chrono::Duration::nanoseconds(1),
        "step must always be positive"
    );
    let mut date_range = Vec::new();
    let mut current_start = start;

    while current_start < end {
        // saturating add, since `step` is expected to be positive
        let next_start = current_start
            .checked_add_signed(max_step)
            .unwrap_or(DateTime::<Utc>::MAX_UTC)
            .min(end); // finish the ranges right at the end
        date_range.push(current_start..next_start);
        current_start = next_start;
    }

    date_range
}

#[cfg(test)]
mod tests {
    use crate::tests::point_construction::{d, dt};

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_generate_date_ranges() {
        for ((from, to), expected) in [
            (
                (day_start(d("2022-01-01")), day_start(d("2022-03-15"))),
                vec![
                    (day_start(d("2022-01-01")), day_start(d("2022-01-31"))),
                    (day_start(d("2022-01-31")), day_start(d("2022-03-02"))),
                    (day_start(d("2022-03-02")), day_start(d("2022-03-15"))),
                ],
            ),
            (
                (day_start(d("2015-07-20")), day_start(d("2016-01-01"))),
                vec![
                    (day_start(d("2015-07-20")), day_start(d("2015-08-19"))),
                    (day_start(d("2015-08-19")), day_start(d("2015-09-18"))),
                    (day_start(d("2015-09-18")), day_start(d("2015-10-18"))),
                    (day_start(d("2015-10-18")), day_start(d("2015-11-17"))),
                    (day_start(d("2015-11-17")), day_start(d("2015-12-17"))),
                    (day_start(d("2015-12-17")), day_start(d("2016-01-01"))),
                ],
            ),
            (
                (day_start(d("2015-07-20")), day_start(d("2015-07-20"))),
                vec![],
            ),
            (
                (
                    dt("2015-07-20T12:12:12").and_utc(),
                    dt("2015-07-20T20:20:20").and_utc(),
                ),
                vec![(
                    dt("2015-07-20T12:12:12").and_utc(),
                    dt("2015-07-20T20:20:20").and_utc(),
                )],
            ),
        ] {
            let expected: Vec<_> = expected.into_iter().map(|r| r.0..r.1).collect();
            let actual = generate_date_time_ranges(from, to, chrono::Duration::days(30));
            assert_eq!(expected, actual);
        }

        // 1 day batch
        assert_eq!(
            vec![day_start(d("2015-07-20"))..day_start(d("2015-07-21"))],
            generate_date_time_ranges(
                day_start(d("2015-07-20")),
                day_start(d("2015-07-21")),
                chrono::Duration::days(1)
            )
        );
        assert_eq!(
            vec![
                day_start(d("2015-07-20"))..day_start(d("2015-07-21")),
                day_start(d("2015-07-21"))..day_start(d("2015-07-22"))
            ],
            generate_date_time_ranges(
                day_start(d("2015-07-20")),
                day_start(d("2015-07-22")),
                chrono::Duration::days(1)
            )
        )
    }

    mod test_batch_step_receives_correct_data {
        use std::{
            ops::Deref,
            str::FromStr,
            sync::{Arc, Mutex, OnceLock},
        };

        use chrono::{DateTime, Days, Utc};
        use entity::sea_orm_active_enums::ChartType;
        use pretty_assertions::assert_eq;

        use crate::{
            data_source::{
                kinds::local_db::{
                    parameters::{
                        update::batching::{parameters::mock::RecordingPassStep, BatchUpdate},
                        DefaultCreate, DefaultQueryVec,
                    },
                    LocalDbChartSource,
                },
                types::Get,
            },
            gettable_const,
            lines::AccountsGrowth,
            tests::{
                point_construction::d,
                recorder::InMemoryRecorder,
                simple_test::{
                    dirty_force_update_and_check, map_str_tuple_to_owned, simple_test_chart,
                },
            },
            ChartProperties, DateValueString, MissingDatePolicy, Named,
        };

        use super::parameters::mock::StepInput;

        type VecStringStepInput = StepInput<Vec<DateValueString>, ()>;
        type SharedInputsStorage = Arc<Mutex<Vec<VecStringStepInput>>>;

        static INPUTS: OnceLock<SharedInputsStorage> = OnceLock::new();

        gettable_const!(ThisInputsStorage: SharedInputsStorage = INPUTS.get_or_init(|| Arc::new(Mutex::new(vec![]))).clone());
        gettable_const!(Batch1Day: chrono::Duration = chrono::Duration::days(1));

        type ThisRecordingStep =
            RecordingPassStep<InMemoryRecorder<VecStringStepInput, ThisInputsStorage>>;
        struct ThisTestChartProps;

        impl Named for ThisTestChartProps {
            const NAME: &'static str = "test_batch_step_receives_correct_data_chart";
        }

        impl ChartProperties for ThisTestChartProps {
            fn chart_type() -> ChartType {
                ChartType::Line
            }
            fn missing_date_policy() -> MissingDatePolicy {
                MissingDatePolicy::FillPrevious
            }
        }

        type ThisRecordingBatchUpdate = BatchUpdate<
            AccountsGrowth,
            (),
            ThisRecordingStep,
            Batch1Day,
            DefaultQueryVec<ThisTestChartProps>,
            ThisTestChartProps,
        >;

        type RecordingChart = LocalDbChartSource<
            AccountsGrowth,
            (),
            DefaultCreate<ThisTestChartProps>,
            ThisRecordingBatchUpdate,
            DefaultQueryVec<ThisTestChartProps>,
            ThisTestChartProps,
        >;

        fn repeat_value_for_dates(
            from: &'static str,
            to: &'static str,
            value: &'static str,
        ) -> Vec<(String, String)> {
            let from = d(from);
            let to = d(to);
            let mut result = Vec::new();
            let mut next_date = from;
            while next_date <= to {
                result.push((next_date.to_string(), value.to_owned()));
                next_date = next_date.checked_add_days(Days::new(1)).unwrap();
            }
            result
        }

        fn clear_inputs(storage: SharedInputsStorage) {
            storage.lock().unwrap().clear();
        }

        fn verify_inputs(
            storage: SharedInputsStorage,
            expected_update_time: Option<DateTime<Utc>>,
        ) {
            let mut prev_input: Option<&StepInput<Vec<DateValueString>, ()>> = None;
            let expected_update_time = expected_update_time
                .unwrap_or(DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap());
            for input in storage.lock().unwrap().deref() {
                assert_eq!(input.chart_id, 3);
                assert_eq!(input.update_time, expected_update_time);
                assert_eq!(input.min_blockscout_block, 0);
                // batch step = 1 day
                assert_eq!(input.main_data.len(), 1);
                if let Some(prev_input) = prev_input {
                    assert_eq!(&input.last_accurate_point, &prev_input.main_data[0]);
                    assert_eq!(
                        input
                            .last_accurate_point
                            .date
                            .checked_add_days(Days::new(1))
                            .unwrap(),
                        input.main_data[0].date
                    );
                }
                prev_input = Some(input);
            }
        }

        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn test_batch_step_receives_correct_data() {
            let expected_data_owned = [
                map_str_tuple_to_owned(vec![
                    ("2022-11-09", "1"),
                    ("2022-11-10", "4"),
                    ("2022-11-11", "8"),
                ]),
                repeat_value_for_dates("2022-11-12", "2023-02-28", "8"),
                map_str_tuple_to_owned(vec![("2023-03-01", "9")]),
            ]
            .concat();
            let expected_data: Vec<(&str, &str)> = expected_data_owned
                .iter()
                .map(|p| (p.0.as_str(), p.1.as_str()))
                .collect();

            let (db, blockscout) = simple_test_chart::<RecordingChart>(
                "test_batch_step_receives_correct_data",
                expected_data.clone(),
            )
            .await;
            verify_inputs(ThisInputsStorage::get(), None);
            clear_inputs(ThisInputsStorage::get());

            // force update should work when existing data is present
            let later_time = DateTime::<Utc>::from_str("2023-03-01T12:00:01Z").unwrap();
            dirty_force_update_and_check::<RecordingChart>(
                &db,
                &blockscout,
                expected_data,
                Some(later_time),
            )
            .await;
            verify_inputs(ThisInputsStorage::get(), Some(later_time));
        }
    }
}
