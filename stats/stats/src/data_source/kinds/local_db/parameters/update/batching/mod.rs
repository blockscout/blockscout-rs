//! Batching for update process.
//!
//! Update for some period P can be done only with dependencies'
//! data for the same exact period P.

use std::{fmt::Debug, marker::PhantomData, ops::Range, time::Instant};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use parameter_traits::BatchStepBehaviour;

use crate::{
    charts::db_interaction::read::get_min_date_blockscout,
    data_source::{
        kinds::local_db::{parameter_traits::QueryBehaviour, UpdateBehaviour},
        source::DataSource,
        types::Get,
        UpdateContext,
    },
    types::{Timespan, TimespanDuration, TimespanValue},
    ChartProperties, UpdateError,
};

pub mod parameter_traits;
pub mod parameters;

pub struct BatchUpdate<MainDep, BatchStep, BatchSizeUpperBound, Query, ChartProps>(
    PhantomData<(MainDep, BatchStep, BatchSizeUpperBound, Query, ChartProps)>,
)
where
    MainDep: DataSource,
    BatchStep: BatchStepBehaviour<ChartProps::Resolution, MainDep::Output>,
    BatchSizeUpperBound: Get<Value = TimespanDuration<ChartProps::Resolution>>,
    Query: QueryBehaviour<Output = Vec<TimespanValue<ChartProps::Resolution, String>>>,
    ChartProps: ChartProperties;

impl<MainDep, BatchStep, BatchSizeUpperBound, Query, ChartProps>
    UpdateBehaviour<MainDep, ChartProps::Resolution>
    for BatchUpdate<MainDep, BatchStep, BatchSizeUpperBound, Query, ChartProps>
where
    MainDep: DataSource,
    BatchStep: BatchStepBehaviour<ChartProps::Resolution, MainDep::Output>,
    BatchSizeUpperBound: Get<Value = TimespanDuration<ChartProps::Resolution>>,
    Query: QueryBehaviour<Output = Vec<TimespanValue<ChartProps::Resolution, String>>>,
    ChartProps: ChartProperties,
    ChartProps::Resolution: Timespan + Ord + Clone + Debug + Send,
{
    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        last_accurate_point: Option<TimespanValue<ChartProps::Resolution, String>>,
        min_blockscout_block: i64,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let now = cx.time;
        let update_from = last_accurate_point
            .clone()
            .map(|p| p.timespan.saturating_next_timespan());
        let update_range_start = match update_from {
            Some(d) => d,
            None => ChartProps::Resolution::from_date(
                get_min_date_blockscout(cx.blockscout)
                    .await
                    .map(|time| time.date())
                    .map_err(UpdateError::BlockscoutDB)?,
            ),
        };

        let steps = generate_batch_ranges(update_range_start, now, BatchSizeUpperBound::get())?;
        let n = steps.len();

        for (i, range) in steps.into_iter().enumerate() {
            let previous_step_last_point = get_previous_step_last_point::<
                Query,
                ChartProps::Resolution,
            >(cx, range.start().clone())
            .await?;
            tracing::info!(
                range =? range.clone().into_date_time_range(),
                previous_step_last_point =? previous_step_last_point,
                chart =% ChartProps::key(),
                "run {}/{} step of batch update", i + 1, n
            );
            let now = Instant::now();
            let found = batch_update_values_step::<MainDep, BatchStep, ChartProps::Resolution>(
                cx,
                chart_id,
                min_blockscout_block,
                previous_step_last_point,
                range.clone(),
                dependency_data_fetch_timer,
            )
            .await?;
            // for query in `get_previous_step_last_point` to work correctly
            Self::update_metadata(cx.db, chart_id, range.into_date_time_range().end).await?;
            let elapsed: std::time::Duration = now.elapsed();
            tracing::info!(
                found =? found,
                elapsed =? elapsed,
                chart =% ChartProps::key(),
                "{}/{} step of batch done", i + 1, n
            );
        }
        Ok(())
    }
}

/// Errors if no point is found
async fn get_previous_step_last_point<Query, Resolution>(
    cx: &UpdateContext<'_>,
    this_step_start: Resolution,
) -> Result<TimespanValue<Resolution, String>, UpdateError>
where
    Resolution: Timespan + Clone,
    Query: QueryBehaviour<Output = Vec<TimespanValue<Resolution, String>>>,
{
    let previous_step_last_point_timespan = this_step_start.saturating_previous_timespan();
    let last_point_range_values = Query::query_data(
        cx,
        Some(previous_step_last_point_timespan.clone().into_time_range()),
    )
    .await?;
    let previous_step_last_point = last_point_range_values
        .last()
        .cloned()
        // `None` means
        // - if `MissingDatePolicy` is `FillZero` = the value is 0
        // - if `MissingDatePolicy` is `FillPrevious` = no previous value was found at this moment in time = value at the point is 0
        .unwrap_or(TimespanValue::with_zero_value(
            previous_step_last_point_timespan,
        ));
    if last_point_range_values.len() > 1 {
        // return error because it's likely that date in `previous_step_last_point` is incorrect
        return Err(UpdateError::Internal("Retrieved 2 points from previous step; probably an issue with range construction and handling".to_owned()));
    }
    Ok(previous_step_last_point)
}

/// Returns how many records were found
async fn batch_update_values_step<MainDep, BatchStep, Resolution>(
    cx: &UpdateContext<'_>,
    chart_id: i32,
    min_blockscout_block: i64,
    last_accurate_point: TimespanValue<Resolution, String>,
    range: BatchRange<Resolution>,
    dependency_data_fetch_timer: &mut AggregateTimer,
) -> Result<usize, UpdateError>
where
    MainDep: DataSource,
    Resolution: Timespan,
    BatchStep: BatchStepBehaviour<Resolution, MainDep::Output>,
{
    let query_range = range.into_date_time_range();
    let main_data =
        MainDep::query_data(cx, Some(query_range.clone()), dependency_data_fetch_timer).await?;
    let found = BatchStep::batch_update_values_step_with(
        cx.db,
        chart_id,
        cx.time,
        min_blockscout_block,
        last_accurate_point,
        main_data,
    )
    .await?;
    Ok(found)
}

/// Non-inclusive (end point is exlcuded)
#[derive(Debug, Clone, PartialEq, Eq)]
enum BatchRange<Resolution> {
    /// Range in non-divided `Resolution` intervals
    Full(Range<Resolution>),
    /// Range ends on some fraction of the `Resolution` interval.
    /// The range is non-inclusive
    Partial {
        start: Resolution,
        end: DateTime<Utc>,
    },
}

impl<Resolution> BatchRange<Resolution> {
    pub fn start(&self) -> &Resolution {
        match self {
            BatchRange::Full(r) => &r.start,
            BatchRange::Partial { start, end: _ } => start,
        }
    }
}

impl<Resolution: Timespan> BatchRange<Resolution> {
    pub fn into_date_time_range(self) -> Range<DateTime<Utc>> {
        match self {
            BatchRange::Full(Range { start, end }) => {
                start.saturating_start_timestamp()..end.into_time_range().start
            }
            BatchRange::Partial { start, end } => start.saturating_start_timestamp()..end,
        }
    }
}

/// Split the range [`start`, `end`) into multiple
/// with maximum length `step`
fn generate_batch_ranges<Resolution>(
    start: Resolution,
    end: DateTime<Utc>,
    max_step: TimespanDuration<Resolution>,
) -> Result<Vec<BatchRange<Resolution>>, UpdateError>
where
    Resolution: Timespan + Ord + Clone,
{
    if max_step.repeats() == 0 {
        return Err(UpdateError::Internal(
            "Zero maximum batch step is not allowed".into(),
        ));
    }
    let mut date_range = Vec::new();
    let mut current_start = start;

    loop {
        let next_start = current_start.clone().saturating_add(max_step.clone()); // finish the ranges right at the end
        let next_start_timestamp = next_start.saturating_start_timestamp();
        if next_start_timestamp > end {
            if end == Resolution::from_date(end.date_naive()).saturating_start_timestamp() {
                // the last interval can be represented as `Resolution` without
                // any fractions
                let end_timespan = Resolution::from_date(end.date_naive());
                let range = current_start..end_timespan;
                if !range.is_empty() {
                    date_range.push(BatchRange::Full(range));
                }
            } else {
                date_range.push(BatchRange::Partial {
                    start: current_start,
                    end,
                });
            }
            break;
        }
        date_range.push(BatchRange::Full(current_start..next_start.clone()));
        current_start = next_start;
        if next_start_timestamp == end {
            break;
        }
    }

    Ok(date_range)
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::point_construction::{d, dt},
        types::timespans::Week,
        utils::day_start,
    };

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_generate_date_ranges() {
        for ((from, to), expected) in [
            (
                (d("2022-01-01"), day_start(&d("2022-03-15"))),
                vec![
                    BatchRange::Full(d("2022-01-01")..d("2022-01-31")),
                    BatchRange::Full(d("2022-01-31")..d("2022-03-02")),
                    BatchRange::Full(d("2022-03-02")..d("2022-03-15")),
                ],
            ),
            (
                (d("2015-07-20"), day_start(&d("2016-01-01"))),
                vec![
                    BatchRange::Full(d("2015-07-20")..d("2015-08-19")),
                    BatchRange::Full(d("2015-08-19")..d("2015-09-18")),
                    BatchRange::Full(d("2015-09-18")..d("2015-10-18")),
                    BatchRange::Full(d("2015-10-18")..d("2015-11-17")),
                    BatchRange::Full(d("2015-11-17")..d("2015-12-17")),
                    BatchRange::Full(d("2015-12-17")..d("2016-01-01")),
                ],
            ),
            ((d("2015-07-20"), day_start(&d("2015-07-20"))), vec![]),
            (
                (d("2015-07-20"), dt("2015-07-20T20:20:20").and_utc()),
                vec![BatchRange::Partial {
                    start: d("2015-07-20"),
                    end: dt("2015-07-20T20:20:20").and_utc(),
                }],
            ),
        ] {
            // let expected: Vec<_> = expected.into_iter().map(|r| r.0..r.1).collect();
            let actual = generate_batch_ranges(from, to, TimespanDuration::from_days(30)).unwrap();
            assert_eq!(expected, actual);
        }

        // 1 day batch
        assert_eq!(
            vec![BatchRange::Full(d("2015-07-20")..d("2015-07-21"))],
            generate_batch_ranges(
                d("2015-07-20"),
                day_start(&d("2015-07-21")),
                TimespanDuration::from_days(1)
            )
            .unwrap()
        );
        assert_eq!(
            vec![
                BatchRange::Full(d("2015-07-20")..d("2015-07-21")),
                BatchRange::Full(d("2015-07-21")..d("2015-07-22"))
            ],
            generate_batch_ranges(
                d("2015-07-20"),
                day_start(&d("2015-07-22")),
                TimespanDuration::from_days(1)
            )
            .unwrap()
        )
    }

    #[test]
    fn batch_range_into_timestamp_range_works() {
        assert_eq!(
            BatchRange::Full(d("2015-11-09")..d("2015-11-09")).into_date_time_range(),
            dt("2015-11-09T00:00:00").and_utc()..dt("2015-11-09T00:00:00").and_utc()
        );
        assert_eq!(
            BatchRange::Full(d("2015-11-09")..d("2015-11-10")).into_date_time_range(),
            dt("2015-11-09T00:00:00").and_utc()..dt("2015-11-10T00:00:00").and_utc()
        );
        assert_eq!(
            BatchRange::Full(d("2015-11-09")..d("2015-11-13")).into_date_time_range(),
            dt("2015-11-09T00:00:00").and_utc()..dt("2015-11-13T00:00:00").and_utc()
        );
        assert_eq!(
            BatchRange::Full(Week::from_date(d("2015-11-09"))..Week::from_date(d("2015-11-19")))
                .into_date_time_range(),
            dt("2015-11-09T00:00:00").and_utc()..dt("2015-11-16T00:00:00").and_utc()
        );
        assert_eq!(
            BatchRange::Partial {
                start: d("2015-11-09"),
                end: dt("2015-11-10T09:12:34").and_utc()
            }
            .into_date_time_range(),
            dt("2015-11-09T00:00:00").and_utc()..dt("2015-11-10T09:12:34").and_utc()
        );
    }

    mod test_batch_step_receives_correct_data {
        use std::{
            str::FromStr,
            sync::{Arc, OnceLock},
        };

        use chrono::{DateTime, Days, NaiveDate, Utc};
        use entity::sea_orm_active_enums::ChartType;
        use pretty_assertions::assert_eq;
        use tokio::sync::Mutex;

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
            types::timespans::DateValue,
            ChartProperties, MissingDatePolicy, Named,
        };

        use super::{parameters::mock::StepInput, TimespanDuration};

        type VecStringStepInput = StepInput<Vec<DateValue<String>>>;
        type SharedInputsStorage = Arc<Mutex<Vec<VecStringStepInput>>>;

        // `OnceLock` in order to return the same instance each time
        static INPUTS: OnceLock<SharedInputsStorage> = OnceLock::new();

        gettable_const!(ThisInputs: SharedInputsStorage = INPUTS.get_or_init(|| Arc::new(Mutex::new(vec![]))).clone());
        gettable_const!(Batch1Day: TimespanDuration<NaiveDate> = TimespanDuration::from_days(1));

        type ThisRecordingStep = RecordingPassStep<InMemoryRecorder<ThisInputs>>;
        struct ThisTestChartProps;

        impl Named for ThisTestChartProps {
            fn name() -> String {
                "test_batch_step_receives_correct_data_chart".into()
            }
        }

        impl ChartProperties for ThisTestChartProps {
            type Resolution = NaiveDate;

            fn chart_type() -> ChartType {
                ChartType::Line
            }
            fn missing_date_policy() -> MissingDatePolicy {
                MissingDatePolicy::FillPrevious
            }
        }

        type ThisRecordingBatchUpdate = BatchUpdate<
            AccountsGrowth,
            ThisRecordingStep,
            Batch1Day,
            DefaultQueryVec<ThisTestChartProps>,
            ThisTestChartProps,
        >;

        type RecordingChart = LocalDbChartSource<
            AccountsGrowth,
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

        async fn clear_inputs(storage: SharedInputsStorage) {
            storage.lock().await.clear();
        }

        async fn verify_inputs(
            storage: SharedInputsStorage,
            expected_update_time: Option<DateTime<Utc>>,
        ) {
            let mut prev_input: Option<&StepInput<Vec<DateValue<String>>>> = None;
            let expected_update_time = expected_update_time
                .unwrap_or(DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap());
            for input in storage.lock().await.deref() {
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
                            .timespan
                            .checked_add_days(Days::new(1))
                            .unwrap(),
                        input.main_data[0].timespan
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
            verify_inputs(ThisInputs::get(), None).await;
            clear_inputs(ThisInputs::get()).await;

            // force update should work when existing data is present
            let later_time = DateTime::<Utc>::from_str("2023-03-01T12:00:01Z").unwrap();
            dirty_force_update_and_check::<RecordingChart>(
                &db,
                &blockscout,
                expected_data,
                Some(later_time),
            )
            .await;
            verify_inputs(ThisInputs::get(), Some(later_time)).await;
        }
    }
}
