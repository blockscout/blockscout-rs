//! Constructors for lower resolutions taking last value within the range.
//!
//! Intended for "growth" charts where cumulative number of something
//! is presented.

use std::{fmt::Debug, marker::PhantomData};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    data_source::{DataSource, UpdateContext},
    range::UniversalRange,
    types::{ConsistsOf, Timespan, TimespanValue},
    ChartError,
};

use super::{extend_to_timespan_boundaries, reduce_each_timespan};

/// Takes last value within each timespan range.
pub struct LastValueLowerResolution<DS, LowerRes>(PhantomData<(DS, LowerRes)>);

impl<DS, LowerRes, HigherRes, Value> DataSource for LastValueLowerResolution<DS, LowerRes>
where
    LowerRes: Timespan + ConsistsOf<HigherRes> + Eq + Ord + Send,
    HigherRes: Clone,
    Value: Send + Debug,
    DS: DataSource<Output = Vec<TimespanValue<HigherRes, Value>>>,
{
    type MainDependencies = DS;
    type ResolutionDependencies = ();
    type Output = Vec<TimespanValue<LowerRes, Value>>;

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

    async fn set_next_update_from_itself(
        _db: &DatabaseConnection,
        _update_from: chrono::NaiveDate,
    ) -> Result<(), ChartError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        let time_range_for_lower_res = extend_to_timespan_boundaries::<LowerRes>(range);
        let high_res_data =
            DS::query_data(cx, time_range_for_lower_res, dependency_data_fetch_timer).await?;
        Ok(reduce_each_timespan(
            high_res_data,
            |t| LowerRes::from_smaller(t.timespan.clone()),
            |a| {
                let last = a.into_iter().next_back();
                last.map(|p| TimespanValue {
                    timespan: LowerRes::from_smaller(p.timespan),
                    value: p.value,
                })
            },
        )
        .into_iter()
        .flatten()
        .collect())
    }
}

#[cfg(test)]
mod tests {
    use blockscout_metrics_tools::AggregateTimer;
    use pretty_assertions::assert_eq;

    use crate::{
        data_source::{types::BlockscoutMigrations, DataSource, UpdateContext, UpdateParameters},
        gettable_const,
        lines::PredefinedMockSource,
        range::UniversalRange,
        tests::point_construction::{d_v_int, dt, w_v_int},
        types::timespans::{DateValue, Week},
        MissingDatePolicy,
    };

    use super::LastValueLowerResolution;

    #[tokio::test]
    async fn last_value_weekly_works() {
        // weeks for this month (2024-07) are
        // 8-14, 15-21, 22-28
        gettable_const!(MockData: Vec<DateValue<i64>> = vec![
            d_v_int("2024-07-08", 1),
            d_v_int("2024-07-12", 3),
            d_v_int("2024-07-27", 5),
            d_v_int("2024-07-28", 1234),
        ]);
        gettable_const!(PolicyGrowth: MissingDatePolicy = MissingDatePolicy::FillPrevious);

        type MockSource = PredefinedMockSource<MockData, PolicyGrowth>;

        type MockSourceWeekly = LastValueLowerResolution<MockSource, Week>;

        // db is not used in mock
        let empty_db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

        let context = UpdateContext::from_params_now_or_override(UpdateParameters {
            db: &empty_db,
            blockscout: &empty_db,
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(dt("2024-07-30T09:00:00").and_utc()),
            force_full: false,
        });
        assert_eq!(
            MockSource::query_data(&context, UniversalRange::full(), &mut AggregateTimer::new())
                .await
                .unwrap(),
            vec![
                d_v_int("2024-07-08", 1),
                d_v_int("2024-07-12", 3),
                d_v_int("2024-07-27", 5),
                d_v_int("2024-07-28", 1234),
            ]
        );
        assert_eq!(
            MockSourceWeekly::query_data(
                &context,
                UniversalRange::full(),
                &mut AggregateTimer::new()
            )
            .await
            .unwrap(),
            vec![w_v_int("2024-07-08", 3), w_v_int("2024-07-22", 1234),]
        );
    }
}
