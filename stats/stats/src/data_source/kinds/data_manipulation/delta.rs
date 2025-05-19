//! The opposite of [cumulative chart](crate::data_source::kinds::local_db::DailyCumulativeLocalDbChartSource).
//! However, it can also work on remotely stored sources, thus different location of the type.
//!
//! I.e. chart "New accounts" is a delta of  "Total accounts".

use std::{
    fmt::Display,
    marker::PhantomData,
    ops::{RangeBounds, SubAssign},
    str::FromStr,
};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::prelude::Zero;

use crate::{
    data_processing::deltas,
    data_source::{kinds::AdapterDataSource, DataSource, UpdateContext},
    range::UniversalRange,
    types::{Timespan, TimespanValue},
    utils::day_start,
    ChartError,
};

/// Calculate delta data from cumulative dependency.
///
/// Missing points in dependency's output are expected to mean "the value is the
/// same as in previous point" (==`MissingDatePolicy::FillPrevious`).
/// [see "Dependency requirements" here](crate::data_source::kinds)
///
/// See [module-level documentation](self) for more details.
pub struct Delta<DS>(PhantomData<DS>)
where
    DS: DataSource;

impl<DS, Resolution, Value> AdapterDataSource for Delta<DS>
where
    DS: DataSource<Output = Vec<TimespanValue<Resolution, Value>>>,
    Resolution: Timespan + Clone + Send,
    Value: FromStr + SubAssign + Zero + Clone + Display + Send,
    TimespanValue<Resolution, Value>: Default,
    <Value as FromStr>::Err: Display,
{
    type MainDependencies = DS;
    type ResolutionDependencies = ();
    type Output = Vec<TimespanValue<Resolution, Value>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        let mut request_range = range.clone();
        // needed to compute deltas for the whole range
        request_range.start = request_range.start.map(|s| {
            s.checked_sub_signed(TimeDelta::days(1))
                .unwrap_or(DateTime::<Utc>::MIN_UTC)
        });
        let start_is_bounded = request_range.start.is_some();
        let cum_data = DS::query_data(cx, request_range, dependency_data_fetch_timer).await?;
        let first_point_is_outside_range = cum_data
            .first()
            .is_some_and(|first| !range.contains(&day_start(&first.timespan.clone().into_date())));
        let (prev_value, cum_data) = if start_is_bounded && first_point_is_outside_range {
            let mut cum_data = cum_data.into_iter();
            let Some(range_start) = cum_data.next() else {
                tracing::warn!("Value before the range was not found, finishing update");
                return Ok(vec![]);
            };
            tracing::debug!(
                range_start_value = %range_start.value,
                cumulative_points_len = cum_data.len(),
                "calculating deltas from cumulative"
            );
            (range_start.value, cum_data.collect())
        } else {
            (Value::zero(), cum_data)
        };
        tracing::debug!(
            prev_value = %prev_value,
            cumulative_points_len = cum_data.len(),
            "calculating deltas from cumulative"
        );
        deltas::<Resolution, Value>(cum_data, prev_value)
    }
}
