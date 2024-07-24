/// Non-daily average values charts
use std::{cmp::Ordering, marker::PhantomData, ops::Range};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use itertools::{EitherOrBoth, Itertools};
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    data_source::{
        kinds::data_manipulation::resolutions::reduce_each_timespan, DataSource, UpdateContext,
    },
    types::{
        week::{Week, WeekValue},
        DateValue, Timespan, TimespanValue,
    },
    UpdateError,
};

use super::extend_to_timespan_boundaries;

/// `DailyWeight` - weight of each day in average source.
/// I.e. if it's average over blocks, then weight is daily number of blocks.
///
/// `DailyAverage` and `DailyWeight`'s missing date values are expected to mean zero
/// (`FillZero`).
/// [see "Dependency requirements" here](crate::data_source::kinds)
pub struct WeeklyAverage<DailyAverage, DailyWeight>(PhantomData<(DailyAverage, DailyWeight)>)
where
    DailyAverage: DataSource<Output = Vec<DateValue<f64>>>,
    DailyWeight: DataSource<Output = Vec<DateValue<i64>>>;

impl<DailyAverage, DailyWeight> DataSource for WeeklyAverage<DailyAverage, DailyWeight>
where
    DailyAverage: DataSource<Output = Vec<DateValue<f64>>>,
    DailyWeight: DataSource<Output = Vec<DateValue<i64>>>,
{
    type MainDependencies = DailyAverage;
    type ResolutionDependencies = DailyWeight;
    type Output = Vec<WeekValue<f64>>;

    fn mutex_id() -> Option<String> {
        // just an adapter
        None
    }

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
        let time_range_for_weeks = range.map(extend_to_timespan_boundaries::<Week>);
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

/// "zip" two sorted date/value vectors, combining
/// values with the same date.
///
/// If both vectors contain values for a date, it yields two values via `EitherOrBoth::Both`.
///
/// If only one of the vectors contains a value for a date, it yields the value via `EitherOrBoth::Left`
/// or `EitherOrBoth::Right`.
fn zip_same_timespan<T, LeftValue, RightValue>(
    left: Vec<TimespanValue<T, LeftValue>>,
    right: Vec<TimespanValue<T, RightValue>>,
) -> Vec<(T, EitherOrBoth<LeftValue, RightValue>)>
where
    T: Ord,
{
    let mut left = left.into_iter().peekable();
    let mut right = right.into_iter().peekable();
    let mut result = vec![];
    loop {
        match (left.peek(), right.peek()) {
            (Some(l), Some(r)) => {
                let (left_t, right_t) = (&l.timespan, &r.timespan);
                match left_t.cmp(right_t) {
                    Ordering::Equal => {
                        let (l, r) = (
                            left.next().expect("peek just succeeded"),
                            right.next().expect("peek just succeeded"),
                        );
                        result.push((l.timespan, EitherOrBoth::Both(l.value, r.value)))
                    }
                    Ordering::Less => {
                        let left_point = left.next().expect("peek just succeeded");
                        result.push((left_point.timespan, EitherOrBoth::Left(left_point.value)))
                    }
                    Ordering::Greater => {
                        let right_point = right.next().expect("peek just succeeded");
                        result.push((right_point.timespan, EitherOrBoth::Right(right_point.value)))
                    }
                }
            }
            (Some(_), None) => {
                result.extend(left.map(|p| (p.timespan, EitherOrBoth::Left(p.value))));
                break;
            }
            (None, Some(_)) => {
                result.extend(right.map(|p| (p.timespan, EitherOrBoth::Right(p.value))));
                break;
            }
            (None, None) => break,
        }
    }
    result
}

fn weekly_average_from(
    day_average: Vec<DateValue<f64>>,
    day_weight: Vec<DateValue<i64>>,
) -> Vec<WeekValue<f64>> {
    // missing points mean zero, treat them as such
    let combined_data = zip_same_timespan(day_average, day_weight);
    let weekly_averages = reduce_each_timespan(
        combined_data,
        |(date, _)| Week::from_date(date.clone()),
        |week_data| {
            let Some((first_date, _)) = week_data.first() else {
                return None;
            };
            let current_week = Week::new(first_date.clone());
            let mut weight_times_avg_sum = 0f64;
            let mut total_weight = 0;
            for (date, values) in week_data {
                debug_assert_eq!(
                    current_week,
                    Week::from_date(date),
                    "must've returned only data within current week ({:?}); got {}",
                    current_week,
                    date
                );
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
            Some(TimespanValue {
                timespan: current_week,
                value: weight_times_avg_sum / total_weight as f64,
            })
        },
    );
    weekly_averages.into_iter().filter_map(|x| x).collect_vec()
}

#[cfg(test)]
mod tests {
    use crate::{
        data_source::kinds::data_manipulation::map::MapParseTo,
        gettable_const,
        lines::PseudoRandomMockRetrieve,
        tests::point_construction::{d, dt, v, v_double, v_int, week_of, week_v_double},
        MissingDatePolicy,
    };

    use super::*;

    use chrono::NaiveDate;
    use itertools::Itertools;
    use pretty_assertions::assert_eq;

    #[test]
    fn zip_same_timespan_works() {
        assert_eq!(
            zip_same_timespan::<NaiveDate, i64, String>(vec![], vec![]),
            vec![]
        );
        assert_eq!(
            zip_same_timespan::<NaiveDate, i64, _>(
                vec![],
                vec![
                    v("2024-07-05", "5R"),
                    v("2024-07-07", "7R"),
                    v("2024-07-08", "8R"),
                    v("2024-07-11", "11R"),
                ]
            ),
            vec![
                (d("2024-07-05"), EitherOrBoth::Right("5R".to_string())),
                (d("2024-07-07"), EitherOrBoth::Right("7R".to_string())),
                (d("2024-07-08"), EitherOrBoth::Right("8R".to_string())),
                (d("2024-07-11"), EitherOrBoth::Right("11R".to_string())),
            ]
        );
        assert_eq!(
            zip_same_timespan::<NaiveDate, _, i64>(
                vec![
                    v("2024-07-05", "5L"),
                    v("2024-07-07", "7L"),
                    v("2024-07-08", "8L"),
                    v("2024-07-11", "11L"),
                ],
                vec![]
            ),
            vec![
                (d("2024-07-05"), EitherOrBoth::Left("5L".to_string())),
                (d("2024-07-07"), EitherOrBoth::Left("7L".to_string())),
                (d("2024-07-08"), EitherOrBoth::Left("8L".to_string())),
                (d("2024-07-11"), EitherOrBoth::Left("11L".to_string())),
            ]
        );
        assert_eq!(
            zip_same_timespan(
                vec![
                    v("2024-07-08", "8L"),
                    v("2024-07-09", "9L"),
                    v("2024-07-10", "10L"),
                ],
                vec![
                    v("2024-07-05", "5R"),
                    v("2024-07-07", "7R"),
                    v("2024-07-08", "8R"),
                    v("2024-07-11", "11R"),
                ]
            ),
            vec![
                (d("2024-07-05"), EitherOrBoth::Right("5R".to_string())),
                (d("2024-07-07"), EitherOrBoth::Right("7R".to_string())),
                (
                    d("2024-07-08"),
                    EitherOrBoth::Both("8L".to_string(), "8R".to_string())
                ),
                (d("2024-07-09"), EitherOrBoth::Left("9L".to_string())),
                (d("2024-07-10"), EitherOrBoth::Left("10L".to_string())),
                (d("2024-07-11"), EitherOrBoth::Right("11R".to_string())),
            ]
        )
    }

    #[test]
    fn weekly_average_from_works() {
        // weeks for this month are
        // 8-14, 15-21, 22-28

        let week_1_average = (5.0 * 100.0 + 34.2 * 2.0 + 10.3 * 12.0) / (100.0 + 2.0 + 12.0);
        assert_eq!(
            weekly_average_from(
                vec![
                    v_double("2024-07-08", 5.0),
                    v_double("2024-07-10", 34.2),
                    v_double("2024-07-14", 10.3),
                    v_double("2024-07-17", 5.0)
                ],
                vec![
                    v_int("2024-07-08", 100),
                    v_int("2024-07-10", 2),
                    v_int("2024-07-14", 12),
                    v_int("2024-07-17", 5)
                ]
            ),
            vec![
                week_v_double("2024-07-08", week_1_average),
                week_v_double("2024-07-15", 5.0)
            ],
        )
    }

    #[tokio::test]
    async fn weekly_average_source_queries_correct_range() {
        gettable_const!(Dates: Range<NaiveDate> = d("2024-07-01")..d("2024-07-31"));
        gettable_const!(RandomAveragesRange: Range<f64> = 1.0..5.0);
        gettable_const!(RandomWeightsRange: Range<u64> = 1..5);
        gettable_const!(Policy: MissingDatePolicy = MissingDatePolicy::FillZero);

        type TestedAverageSource = WeeklyAverage<
            MapParseTo<PseudoRandomMockRetrieve<Dates, RandomAveragesRange, f64, Policy>, f64>,
            MapParseTo<PseudoRandomMockRetrieve<Dates, RandomWeightsRange, u64, Policy>, i64>,
        >;

        // weeks for this month are
        // 8-14, 15-21, 22-28

        // db is not used in mock
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let output: Vec<WeekValue<f64>> = TestedAverageSource::query_data(
            &UpdateContext {
                db: &db,
                blockscout: &db,
                time: dt("2024-07-15T09:00:00").and_utc(),
                force_full: false,
            },
            Some(dt("2024-07-08T09:00:00").and_utc()..dt("2024-07-15T00:00:01").and_utc()),
            &mut AggregateTimer::new(),
        )
        .await
        .unwrap();
        assert_eq!(
            output
                .into_iter()
                .map(|week_value| week_value.timespan)
                .collect_vec(),
            vec![week_of("2024-07-08"), week_of("2024-07-15"),]
        );
    }
}
