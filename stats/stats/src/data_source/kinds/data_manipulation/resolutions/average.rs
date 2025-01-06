//! Constructors for lower resolutions of average value charts
use std::{fmt::Debug, marker::PhantomData};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use itertools::{EitherOrBoth, Itertools};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    data_processing::zip_same_timespan,
    data_source::{
        kinds::data_manipulation::resolutions::reduce_each_timespan, DataSource, UpdateContext,
    },
    range::UniversalRange,
    types::{ConsistsOf, Timespan, TimespanValue},
    ChartError,
};

use super::extend_to_timespan_boundaries;

/// `Weight` - weight of each timespan in average source.
/// E.g. if it's average over blocks, then weight is number of blocks in each point.
///
/// `LowerRes` - target resolution of resulting data source. Lower = longer.
/// E.g. if `LowerRes` is `Week`, then source data is expected to be daily.
///
/// `Average` and `Weight`'s missing date values are expected to mean zero
/// (`FillZero`).
/// [see "Dependency requirements" here](crate::data_source::kinds)
pub struct AverageLowerResolution<Average, Weight, LowerRes>(
    PhantomData<(Average, Weight, LowerRes)>,
);

impl<Average, Weight, LowerRes, HigherRes> DataSource
    for AverageLowerResolution<Average, Weight, LowerRes>
where
    Average: DataSource<Output = Vec<TimespanValue<HigherRes, f64>>>,
    Weight: DataSource<Output = Vec<TimespanValue<HigherRes, i64>>>,
    LowerRes: Timespan + ConsistsOf<HigherRes> + Eq + Ord + Debug + Send,
    HigherRes: Ord + Clone + Debug + Send,
{
    type MainDependencies = Average;
    type ResolutionDependencies = Weight;
    type Output = Vec<TimespanValue<LowerRes, f64>>;

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

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), ChartError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        let time_range_for_lower_res = extend_to_timespan_boundaries::<LowerRes>(range);
        let high_res_averages = Average::query_data(
            cx,
            time_range_for_lower_res.clone(),
            dependency_data_fetch_timer,
        )
        .await?;
        let weights =
            Weight::query_data(cx, time_range_for_lower_res, dependency_data_fetch_timer).await?;
        Ok(lower_res_average_from(high_res_averages, weights))
    }
}

fn lower_res_average_from<LowerRes, HigherRes>(
    h_res_average: Vec<TimespanValue<HigherRes, f64>>,
    h_res_weight: Vec<TimespanValue<HigherRes, i64>>,
) -> Vec<TimespanValue<LowerRes, f64>>
where
    LowerRes: ConsistsOf<HigherRes> + Eq + Debug,
    HigherRes: Ord + Clone + Debug,
{
    // missing points mean zero, treat them as such
    let combined_data = zip_same_timespan(h_res_average, h_res_weight);
    let l_res_averages = reduce_each_timespan(
        combined_data,
        |(h_res, _)| LowerRes::from_smaller(h_res.clone()),
        |data_for_one_l_res| {
            let (first_h_res, _) = data_for_one_l_res.first()?;
            let current_l_res = LowerRes::from_smaller(first_h_res.clone());
            let mut weight_times_avg_sum = 0f64;
            let mut total_weight = 0;
            for (h_res, values) in data_for_one_l_res {
                debug_assert_eq!(
                    current_l_res,
                    LowerRes::from_smaller(h_res.clone()),
                    "must've returned only data within current lower res timespan ({:?}); got {:?}",
                    current_l_res,
                    h_res
                );
                match values {
                    EitherOrBoth::Both(avg, weight) => {
                        weight_times_avg_sum += avg * weight as f64;
                        total_weight += weight;
                    }
                    EitherOrBoth::Left(v) => tracing::warn!(
                        value = v,
                        timespan =? h_res,
                        "average value for higher res timespan is present while weight is not (weight is zero).\
                         this is very likely incorrect, please investigate.",
                    ),
                    EitherOrBoth::Right(weight) => {
                        // `avg` is zero, completely possible
                        total_weight += weight
                    }
                }
            }
            if total_weight != 0 {
                Some(TimespanValue {
                    timespan: current_l_res,
                    value: weight_times_avg_sum / total_weight as f64,
                })
            } else {
                None
            }
        },
    );
    l_res_averages.into_iter().flatten().collect_vec()
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use crate::{
        data_source::{
            kinds::data_manipulation::map::MapParseTo, types::BlockscoutMigrations,
            UpdateParameters,
        },
        gettable_const,
        lines::{PredefinedMockSource, PseudoRandomMockRetrieve},
        tests::point_construction::{d, d_v_double, d_v_int, dt, w_v_double, week_of},
        types::timespans::{DateValue, Week, WeekValue},
        MissingDatePolicy,
    };

    use super::*;

    use chrono::NaiveDate;
    use itertools::Itertools;
    use pretty_assertions::assert_eq;

    #[test]
    fn weekly_average_from_works() {
        // weeks for this month are
        // 8-14, 15-21, 22-28

        let week_1_average = (5.0 * 100.0 + 34.2 * 2.0 + 10.3 * 12.0) / (100.0 + 2.0 + 12.0);
        assert_eq!(
            lower_res_average_from(
                vec![
                    d_v_double("2024-07-08", 5.0),
                    d_v_double("2024-07-10", 34.2),
                    d_v_double("2024-07-14", 10.3),
                    d_v_double("2024-07-17", 5.0)
                ],
                vec![
                    d_v_int("2024-07-08", 100),
                    d_v_int("2024-07-10", 2),
                    d_v_int("2024-07-14", 12),
                    d_v_int("2024-07-17", 5)
                ]
            ),
            vec![
                w_v_double("2024-07-08", week_1_average),
                w_v_double("2024-07-15", 5.0)
            ],
        )
    }

    #[tokio::test]
    async fn weekly_average_source_queries_correct_range() {
        gettable_const!(Dates: Range<NaiveDate> = d("2024-07-01")..d("2024-07-31"));
        gettable_const!(RandomAveragesRange: Range<f64> = 1.0..5.0);
        gettable_const!(RandomWeightsRange: Range<u64> = 1..5);
        gettable_const!(Policy: MissingDatePolicy = MissingDatePolicy::FillZero);

        type TestedAverageSource = AverageLowerResolution<
            MapParseTo<PseudoRandomMockRetrieve<Dates, RandomAveragesRange, Policy>, f64>,
            MapParseTo<PseudoRandomMockRetrieve<Dates, RandomWeightsRange, Policy>, i64>,
            Week,
        >;

        // weeks for this month are
        // 8-14, 15-21, 22-28

        // db is not used in mock
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let output: Vec<WeekValue<f64>> = TestedAverageSource::query_data(
            &UpdateContext::from_params_now_or_override(UpdateParameters {
                db: &db,
                blockscout: &db,
                blockscout_applied_migrations: BlockscoutMigrations::latest(),
                update_time_override: Some(dt("2024-07-15T09:00:00").and_utc()),
                force_full: false,
            }),
            (dt("2024-07-08T09:00:00").and_utc()..dt("2024-07-15T00:00:01").and_utc()).into(),
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

    #[tokio::test]
    async fn average_weekly_works() {
        // weeks for this month (2024-07) are
        // 8-14, 15-21, 22-28
        gettable_const!(MockDailyAverage: Vec<DateValue<f64>> = vec![
            d_v_double("2024-07-08", 5.0),
            d_v_double("2024-07-10", 34.2),
            d_v_double("2024-07-14", 10.3),
            d_v_double("2024-07-17", 5.0)
        ]);
        gettable_const!(MockWeights: Vec<DateValue<i64>> = vec![
            d_v_int("2024-07-08", 100),
            d_v_int("2024-07-10", 2),
            d_v_int("2024-07-14", 12),
            d_v_int("2024-07-17", 5)
        ]);
        gettable_const!(Policy: MissingDatePolicy = MissingDatePolicy::FillZero);

        type PredefinedDailyAverage = PredefinedMockSource<MockDailyAverage, Policy>;
        type PredefinedWeights = PredefinedMockSource<MockWeights, Policy>;

        type TestedAverageSource =
            AverageLowerResolution<PredefinedDailyAverage, PredefinedWeights, Week>;

        // db is not used in mock
        let empty_db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

        let context = UpdateContext::from_params_now_or_override(UpdateParameters {
            db: &empty_db,
            blockscout: &empty_db,
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(dt("2024-07-30T09:00:00").and_utc()),
            force_full: false,
        });
        let week_1_average = (5.0 * 100.0 + 34.2 * 2.0 + 10.3 * 12.0) / (100.0 + 2.0 + 12.0);
        assert_eq!(
            TestedAverageSource::query_data(
                &context,
                UniversalRange::full(),
                &mut AggregateTimer::new()
            )
            .await
            .unwrap(),
            vec![
                w_v_double("2024-07-08", week_1_average),
                w_v_double("2024-07-15", 5.0)
            ]
        );
    }

    #[tokio::test]
    async fn average_weekly_works_with_missing_avg() {
        let _ = tracing_subscriber::fmt::try_init();

        gettable_const!(MockDailyAverage: Vec<DateValue<f64>> = vec![
            d_v_double("2022-11-09", 1.0),
            d_v_double("2022-11-10", 1.0),
            d_v_double("2022-11-11", 1.0),
            // missing average for 2022-11-12 should be treated as 0
        ]);
        gettable_const!(MockWeights: Vec<DateValue<i64>> = vec![
            d_v_int("2022-11-09", 1),
            d_v_int("2022-11-10", 3),
            d_v_int("2022-11-11", 4),
            d_v_int("2022-11-12", 1),
        ]);
        gettable_const!(Policy: MissingDatePolicy = MissingDatePolicy::FillZero);

        type PredefinedDailyAverage = PredefinedMockSource<MockDailyAverage, Policy>;
        type PredefinedWeights = PredefinedMockSource<MockWeights, Policy>;

        type TestedAverageSource =
            AverageLowerResolution<PredefinedDailyAverage, PredefinedWeights, Week>;

        // db is not used in mock
        let empty_db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

        let context = UpdateContext::from_params_now_or_override(UpdateParameters {
            db: &empty_db,
            blockscout: &empty_db,
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(dt("2023-03-30T09:00:00").and_utc()),
            force_full: false,
        });
        assert_eq!(
            TestedAverageSource::query_data(
                &context,
                UniversalRange::full(),
                &mut AggregateTimer::new()
            )
            .await
            .unwrap(),
            vec![w_v_double("2022-11-07", 0.8888888888888888),]
        );
    }

    #[tokio::test]
    async fn average_weekly_works_with_missing_weight() {
        let _ = tracing_subscriber::fmt::try_init();

        gettable_const!(MockDailyAverage: Vec<DateValue<f64>> = vec![
            d_v_double("2022-11-09", 1.0),
            d_v_double("2022-11-10", 1.0),
            d_v_double("2022-11-11", 1.0),
            d_v_double("2022-11-12", 1.0),
        ]);
        gettable_const!(MockWeights: Vec<DateValue<i64>> = vec![
            d_v_int("2022-11-09", 1),
            d_v_int("2022-11-10", 3),
            d_v_int("2022-11-11", 4),
            // missing weight for 2022-11-12 is not valid and will be ignored (with warning produced)
        ]);
        gettable_const!(Policy: MissingDatePolicy = MissingDatePolicy::FillZero);

        type PredefinedDailyAverage = PredefinedMockSource<MockDailyAverage, Policy>;
        type PredefinedWeights = PredefinedMockSource<MockWeights, Policy>;

        type TestedAverageSource =
            AverageLowerResolution<PredefinedDailyAverage, PredefinedWeights, Week>;

        // db is not used in mock
        let empty_db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

        let context = UpdateContext::from_params_now_or_override(UpdateParameters {
            db: &empty_db,
            blockscout: &empty_db,
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(dt("2023-03-30T09:00:00").and_utc()),
            force_full: false,
        });
        assert_eq!(
            TestedAverageSource::query_data(
                &context,
                UniversalRange::full(),
                &mut AggregateTimer::new()
            )
            .await
            .unwrap(),
            vec![w_v_double("2022-11-07", 1.0),]
        );
    }
}
