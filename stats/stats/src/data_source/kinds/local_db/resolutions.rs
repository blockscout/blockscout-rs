// todo: refactor

use std::{
    cmp::Ordering,
    marker::PhantomData,
    ops::{Range, RangeInclusive},
};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use itertools::EitherOrBoth;
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    data_source::{DataSource, UpdateContext},
    types::{
        week::{Week, WeekValueDouble},
        DateValueDouble, DateValueInt, TimespanValue,
    },
    utils::{day_start, exclusive_datetime_range_to_inclusive},
    UpdateError,
};

// todo: remove?
pub trait AggregationBehaviour {
    // this chart with other resolution
    type ThisChart: DataSource;
    // other deps needed for the aggregation
    type ResolutionDependencies: DataSource;

    type Output: TimespanValue<Timespan = Week> + Send;
}

/// `DailyWeight` - weight of each day in average source.
/// I.e. if it's average over blocks, then weight is daily number of blocks.
///
/// `DailyAverage` and `DailyWeight`'s missing date values are expected to mean zero
/// (`FillZero`).
/// [see "Dependency requirements" here](crate::data_source::kinds)
pub struct WeeklyAverage<DailyAverage, DailyWeight>(PhantomData<(DailyAverage, DailyWeight)>)
where
    DailyAverage: DataSource<Output = Vec<DateValueDouble>>,
    DailyWeight: DataSource<Output = Vec<DateValueInt>>;

impl<DailyAverage, DailyWeight> DataSource for WeeklyAverage<DailyAverage, DailyWeight>
where
    DailyAverage: DataSource<Output = Vec<DateValueDouble>>,
    DailyWeight: DataSource<Output = Vec<DateValueInt>>,
{
    type MainDependencies = DailyAverage;
    type ResolutionDependencies = DailyWeight;
    type Output = Vec<WeekValueDouble>;

    const MUTEX_ID: Option<&'static str> = None;

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &DateTime<Utc>,
    ) -> Result<(), DbErr> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        let week_range = range.map(date_range_to_weeks);
        let time_range_for_weeks = week_range.map(|w| {
            // start of week containing range start
            let start = day_start(*w.start().days().start());
            // start of week following range end (exclusive range again)
            let week_after_range = w.end().saturating_next_week();
            let end = day_start(*week_after_range.days().start());
            start..end
        });
        let daily_averages = DailyAverage::query_data(
            cx,
            time_range_for_weeks.clone(),
            dependency_data_fetch_timer,
        )
        .await?;
        let weights =
            DailyWeight::query_data(cx, time_range_for_weeks, dependency_data_fetch_timer).await?;
        Ok(weekly_average_from(daily_averages, weights))
    }
}

fn date_range_to_weeks(range: Range<DateTime<Utc>>) -> RangeInclusive<Week> {
    let range = exclusive_datetime_range_to_inclusive(range);
    let start_week = Week::new(range.start().date_naive());
    let end_week = Week::new(range.end().date_naive());
    start_week..=end_week
}

/// "zip" two sorted date/value vectors, combining
/// values with the same date.
///
/// If both vectors contain values for a date, it yields two values via `EitherOrBoth::Both`.
///
/// If only one of the vectors contains a value for a date, it yields the value via `EitherOrBoth::Left`
/// or `EitherOrBoth::Right`.
fn zip_same_timespan<PointLeft, PointRight>(
    left: Vec<PointLeft>,
    right: Vec<PointRight>,
) -> Vec<(
    PointLeft::Timespan,
    EitherOrBoth<PointLeft::Value, PointRight::Value>,
)>
where
    PointLeft: TimespanValue,
    PointRight: TimespanValue<Timespan = PointLeft::Timespan>,
    PointLeft::Timespan: Ord,
{
    let mut left = left.into_iter().peekable();
    let mut right = right.into_iter().peekable();
    let mut result = vec![];
    loop {
        match (left.peek(), right.peek()) {
            (Some(l), Some(r)) => {
                let (left_t, right_t) = (l.get_parts().0, r.get_parts().0);
                match left_t.cmp(right_t) {
                    Ordering::Equal => {
                        let (l, r) = (
                            left.next().expect("peek just succeeded"),
                            right.next().expect("peek just succeeded"),
                        );
                        let (t, left_v) = l.into_parts();
                        let (_, right_v) = r.into_parts();
                        result.push((t, EitherOrBoth::Both(left_v, right_v)))
                    }
                    Ordering::Less => {
                        let (t, left_v) = left.next().expect("peek just succeeded").into_parts();
                        result.push((t, EitherOrBoth::Left(left_v)))
                    }
                    Ordering::Greater => {
                        let (t, right_v) = right.next().expect("peek just succeeded").into_parts();
                        result.push((t, EitherOrBoth::Right(right_v)))
                    }
                }
            }
            (Some(_), None) => {
                result.extend(left.map(|p| {
                    let (t, v) = p.into_parts();
                    (t, EitherOrBoth::Left(v))
                }));
                break;
            }
            (None, Some(_)) => {
                result.extend(right.map(|p| {
                    let (t, v) = p.into_parts();
                    (t, EitherOrBoth::Right(v))
                }));
                break;
            }
            (None, None) => break,
        }
    }
    result
}

fn weekly_average_from(
    daily_average: Vec<DateValueDouble>,
    day_weights: Vec<DateValueInt>,
) -> Vec<WeekValueDouble> {
    // missing points mean zero, treat them as such
    let combined_data = zip_same_timespan(daily_average, day_weights);
    let mut weekly_averages = vec![];
    let Some(mut current_week) = combined_data.first().map(|(w, _)| Week::new(*w)) else {
        return vec![];
    };
    let mut weight_times_avg_sum = 0f64;
    let mut total_weight = 0;

    fn push_weekly_average(
        averages: &mut Vec<WeekValueDouble>,
        week: Week,
        weight_times_avg_sum: &mut f64,
        total_weight: &mut i64,
    ) {
        let value = *weight_times_avg_sum / *total_weight as f64;
        averages.push(WeekValueDouble { week, value });
        *weight_times_avg_sum = 0.0;
        *total_weight = 0;
    }

    for (date, values) in combined_data {
        if current_week != Week::new(date) {
            push_weekly_average(
                &mut weekly_averages,
                current_week,
                &mut weight_times_avg_sum,
                &mut total_weight,
            );
            current_week = Week::new(date);
        }
        match values {
            EitherOrBoth::Both(avg, weight) => {
                weight_times_avg_sum += avg * weight as f64;
                total_weight += weight;
            }
            EitherOrBoth::Left(v) => tracing::warn!(
                value = v,
                "average value for date {} is present while weight is not",
                date
            ),
            EitherOrBoth::Right(v) => tracing::warn!(
                value = v,
                "weight for date {} is present average value is not",
                date
            ),
        }
    }
    push_weekly_average(
        &mut weekly_averages,
        current_week,
        &mut weight_times_avg_sum,
        &mut total_weight,
    );
    weekly_averages
}
