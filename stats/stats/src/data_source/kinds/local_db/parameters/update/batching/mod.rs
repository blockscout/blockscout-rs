//! Batching for update process.
//!
//! Update for some period P can be done only with dependencies'
//! data for the same exact period P.

use std::{marker::PhantomData, ops::RangeInclusive, time::Instant};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Days, Utc};
use parameter_traits::{BatchSizeUpperBound, BatchStepBehaviour};
use sea_orm::TransactionTrait;

use crate::{
    charts::db_interaction::{self, read::get_min_date_blockscout},
    data_source::{kinds::local_db::UpdateBehaviour, source::DataSource, UpdateContext},
    utils::day_start,
    ChartProperties, DateValueString, UpdateError,
};

pub mod parameter_traits;
pub mod parameters;

pub struct BatchUpdate<MainDep, ResolutionDep, BatchStep, BatchSize, ChartProps>(
    PhantomData<(MainDep, ResolutionDep, BatchStep, BatchSize, ChartProps)>,
)
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    BatchStep: BatchStepBehaviour<MainDep::Output, ResolutionDep::Output>,
    BatchSize: BatchSizeUpperBound,
    ChartProps: ChartProperties;

impl<MainDep, ResolutionDep, BatchStep, BatchSize, ChartProps>
    UpdateBehaviour<MainDep, ResolutionDep>
    for BatchUpdate<MainDep, ResolutionDep, BatchStep, BatchSize, ChartProps>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    BatchStep: BatchStepBehaviour<MainDep::Output, ResolutionDep::Output>,
    BatchSize: BatchSizeUpperBound,
    ChartProps: ChartProperties,
{
    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        mut last_accurate_point: Option<DateValueString>,
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

        let steps = generate_date_time_ranges(first_date_time, now, BatchSize::batch_max_size());
        let n = steps.len();

        for (i, range) in steps.into_iter().enumerate() {
            tracing::info!(from =? range.start(), to =? range.end() , "run {}/{} step of batch update", i + 1, n);
            let now = Instant::now();
            let found = batch_update_values_step::<MainDep, ResolutionDep, BatchStep>(
                cx,
                chart_id,
                min_blockscout_block,
                last_accurate_point,
                range,
                dependency_data_fetch_timer,
            )
            .await?;
            let elapsed: std::time::Duration = now.elapsed();
            tracing::info!(found =? found, elapsed =? elapsed, "{}/{} step of batch done", i + 1, n);
            last_accurate_point = db_interaction::read::last_accurate_point::<ChartProps>(
                chart_id,
                min_blockscout_block,
                cx.db,
                false,
                Some(ChartProps::approximate_trailing_points()),
            )
            .await?;
        }
        Ok(())
    }
}

/// Returns how many records were found
async fn batch_update_values_step<MainDep, ResolutionDep, BatchStep>(
    cx: &UpdateContext<'_>,
    chart_id: i32,
    min_blockscout_block: i64,
    last_accurate_point: Option<DateValueString>,
    range: RangeInclusive<DateTime<Utc>>,
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

/// Split the range [`start`, `end`] into multiple
/// with maximum length `step`
fn generate_date_time_ranges(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    max_step: chrono::Duration,
) -> Vec<RangeInclusive<DateTime<Utc>>> {
    let mut date_range = Vec::new();
    let mut current_date_time = start;

    while current_date_time < end {
        // saturating add, since `step` is expected to be positive
        let next_date = current_date_time
            .checked_add_signed(max_step)
            .unwrap_or(DateTime::<Utc>::MAX_UTC)
            .min(end); // finish the ranges right at the end
        date_range.push(RangeInclusive::new(current_date_time, next_date));
        current_date_time = next_date;
    }

    date_range
}

#[cfg(test)]
mod tests {
    use crate::tests::point_construction::{d, dt};

    use super::*;
    use chrono::{NaiveDate, NaiveTime};
    use pretty_assertions::assert_eq;

    // there are leap seconds and such, thus for testing only
    fn day_end_ish(date: NaiveDate) -> DateTime<Utc> {
        date.and_time(NaiveTime::from_hms_opt(23, 59, 59).expect("correct time"))
            .and_utc()
    }

    #[test]
    fn test_generate_date_ranges() {
        for ((from, to), expected) in [
            (
                (day_start(d("2022-01-01")), day_end_ish(d("2022-03-14"))),
                vec![
                    (day_start(d("2022-01-01")), day_start(d("2022-01-31"))),
                    (day_start(d("2022-01-31")), day_start(d("2022-03-02"))),
                    (day_start(d("2022-03-02")), day_end_ish(d("2022-03-14"))),
                ],
            ),
            (
                (day_start(d("2015-07-20")), day_end_ish(d("2015-12-31"))),
                vec![
                    (day_start(d("2015-07-20")), day_start(d("2015-08-19"))),
                    (day_start(d("2015-08-19")), day_start(d("2015-09-18"))),
                    (day_start(d("2015-09-18")), day_start(d("2015-10-18"))),
                    (day_start(d("2015-10-18")), day_start(d("2015-11-17"))),
                    (day_start(d("2015-11-17")), day_start(d("2015-12-17"))),
                    (day_start(d("2015-12-17")), day_end_ish(d("2015-12-31"))),
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
            let expected: Vec<_> = expected
                .into_iter()
                .map(|r| RangeInclusive::new(r.0, r.1))
                .collect();
            let actual = generate_date_time_ranges(from, to, chrono::Duration::days(30));
            assert_eq!(expected, actual);
        }
    }
}
