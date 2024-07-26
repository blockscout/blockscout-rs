//! Construct other resolutions taking last value within the range.
//!
//! Intended for "growth" charts where cumulative number of something
//! is presented.

use std::{fmt::Debug, marker::PhantomData, ops::Range};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    data_source::{DataSource, UpdateContext},
    types::{ConsistsOf, Timespan, TimespanValue},
    UpdateError,
};

use super::{extend_to_timespan_boundaries, reduce_each_timespan};

/// Takes last value within each timespan range.
pub struct LastValueLowerResolution<DS, LowerRes>(PhantomData<(DS, LowerRes)>);

impl<DS, LowerRes, HigherRes, Value> DataSource for LastValueLowerResolution<DS, LowerRes>
where
    LowerRes: Timespan + ConsistsOf<HigherRes> + Eq + Send,
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

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        let time_range_for_lower_res = range.map(extend_to_timespan_boundaries::<LowerRes>);
        let high_res_data = DS::query_data(
            cx,
            time_range_for_lower_res.clone(),
            dependency_data_fetch_timer,
        )
        .await?;
        Ok(reduce_each_timespan(
            high_res_data,
            |t| LowerRes::from_smaller(t.timespan.clone()),
            |a| {
                let last = a.into_iter().rev().next();
                last.map(|p| TimespanValue {
                    timespan: LowerRes::from_smaller(p.timespan),
                    value: p.value,
                })
            },
        )
        .into_iter()
        .filter_map(|x| x)
        .collect())
    }
}

#[cfg(test)]
mod tests {
    use blockscout_metrics_tools::AggregateTimer;
    use pretty_assertions::assert_eq;

    use crate::{
        data_source::{DataSource, UpdateContext},
        gettable_const,
        lines::PredefinedMockSource,
        tests::point_construction::{d_v_int, dt, week_v_int},
        types::{week::Week, DateValue},
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

        let context = UpdateContext {
            db: &empty_db,
            blockscout: &empty_db,
            time: dt("2024-07-30T09:00:00").and_utc(),
            force_full: false,
        };
        assert_eq!(
            MockSource::query_data(&context, None, &mut AggregateTimer::new())
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
            MockSourceWeekly::query_data(&context, None, &mut AggregateTimer::new())
                .await
                .unwrap(),
            vec![week_v_int("2024-07-08", 3), week_v_int("2024-07-22", 1234),]
        );
    }
}
