//! Construct other resolutions taking sum of values within the range.
//!
//! Intended for "new"/"delta" charts where change of something
//! is presented.

use std::{
    fmt::Debug,
    marker::PhantomData,
    ops::{AddAssign, Range},
};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::Zero;
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    data_source::{DataSource, UpdateContext},
    types::{
        week::{Week, WeekValue},
        DateValue, Timespan, TimespanValue,
    },
    UpdateError,
};

use super::{extend_to_timespan_boundaries, reduce_each_timespan};

/// Sum points within each timespan range.
pub struct SumWeekly<DS>(PhantomData<DS>)
where
    DS: DataSource;

impl<DS, Value> DataSource for SumWeekly<DS>
where
    Value: AddAssign + Zero + Send + Debug,
    DS: DataSource<Output = Vec<DateValue<Value>>>,
{
    type MainDependencies = DS;
    type ResolutionDependencies = ();
    type Output = Vec<WeekValue<Value>>;

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
        let daily_data = DS::query_data(
            cx,
            time_range_for_weeks.clone(),
            dependency_data_fetch_timer,
        )
        .await?;
        Ok(reduce_each_timespan(
            daily_data,
            |t| Week::from_date(t.timespan),
            |l_timespan_data| {
                let Some(TimespanValue {
                    timespan: first_date,
                    value: _,
                }) = l_timespan_data.first()
                else {
                    return None;
                };
                let current_l_timespan = Week::new(first_date.clone());
                let mut total = Value::zero();
                for TimespanValue {
                    timespan: s_timespan,
                    value,
                } in l_timespan_data
                {
                    debug_assert_eq!(
                        current_l_timespan,
                        Week::from_date(s_timespan),
                        "must've returned only data within current week ({:?}); got {}",
                        current_l_timespan,
                        s_timespan
                    );
                    total += value;
                }
                Some(TimespanValue {
                    timespan: current_l_timespan,
                    value: total,
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
        types::DateValue,
        MissingDatePolicy,
    };

    use super::SumWeekly;

    #[tokio::test]
    async fn sum_weekly_works() {
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

        type MockSourceWeekly = SumWeekly<MockSource>;

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
            vec![week_v_int("2024-07-08", 4), week_v_int("2024-07-22", 1239),]
        );
    }
}
