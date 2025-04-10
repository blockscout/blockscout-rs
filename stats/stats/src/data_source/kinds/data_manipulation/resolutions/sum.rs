//! Constructors for lower resolutions taking sum of values within the range.
//!
//! Intended for "new"/"delta" charts where change of something
//! is presented.

use std::{fmt::Debug, marker::PhantomData, ops::AddAssign};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::Zero;

use crate::{
    data_source::{kinds::AdapterDataSource, DataSource, UpdateContext},
    range::UniversalRange,
    types::{ConsistsOf, Timespan, TimespanValue},
    ChartError,
};

use super::{extend_to_timespan_boundaries, reduce_each_timespan};

/// Sum points within each timespan range.
pub struct SumLowerResolution<DS, LowerRes>(PhantomData<(DS, LowerRes)>);

impl<DS, LowerRes, HigherRes, Value> AdapterDataSource for SumLowerResolution<DS, LowerRes>
where
    LowerRes: Timespan + ConsistsOf<HigherRes> + Eq + Ord + Debug + Send,
    HigherRes: Clone + Debug,
    Value: AddAssign + Zero + Send + Debug,
    DS: DataSource<Output = Vec<TimespanValue<HigherRes, Value>>>,
{
    type MainDependencies = DS;
    type ResolutionDependencies = ();
    type Output = Vec<TimespanValue<LowerRes, Value>>;

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
            |data_for_one_l_res| {
                let TimespanValue {
                    timespan: first_h_res,
                    value: _,
                } = data_for_one_l_res.first()?;
                let current_l_res = LowerRes::from_smaller(first_h_res.clone());
                let mut total = Value::zero();
                for TimespanValue {
                    timespan: h_res,
                    value,
                } in data_for_one_l_res
                {
                    debug_assert_eq!(
                        current_l_res,
                        LowerRes::from_smaller(h_res.clone()),
                        "must've returned only data within current lower res timespan ({:?}); got {:?}",
                        current_l_res,
                        h_res
                    );
                    total += value;
                }
                Some(TimespanValue {
                    timespan: current_l_res,
                    value: total,
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

    use super::SumLowerResolution;

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

        type MockSourceWeekly = SumLowerResolution<MockSource, Week>;

        // db is not used in mock
        let empty_db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

        let context =
            UpdateContext::from_params_now_or_override(UpdateParameters::query_parameters(
                &empty_db,
                &empty_db,
                BlockscoutMigrations::latest(),
                Some(dt("2024-07-30T09:00:00").and_utc()),
            ));
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
            vec![w_v_int("2024-07-08", 4), w_v_int("2024-07-22", 1239),]
        );
    }
}
