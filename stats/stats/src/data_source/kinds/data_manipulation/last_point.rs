//! Last point data source.
//!
//! Takes last data point from some other (vector) source

use std::marker::PhantomData;

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};

use crate::{
    ChartError,
    data_source::{UpdateContext, kinds::AdapterDataSource, source::DataSource},
    range::UniversalRange,
    types::{Timespan, TimespanValue, ZeroTimespanValue},
    utils::day_start,
};

pub struct LastPoint<DS>(PhantomData<DS>)
where
    DS: DataSource;

impl<DS, Resolution, Value> AdapterDataSource for LastPoint<DS>
where
    Resolution: Timespan + Ord + Send,
    Value: Send,
    DS: DataSource<Output = Vec<TimespanValue<Resolution, Value>>>,
    TimespanValue<Resolution, Value>: ZeroTimespanValue<Resolution>,
{
    type MainDependencies = DS;
    type ResolutionDependencies = ();
    type Output = TimespanValue<Resolution, Value>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        let data = DS::query_data(
            cx,
            (day_start(&cx.time.date_naive())..cx.time).into(),
            dependency_data_fetch_timer,
        )
        .await?;
        tracing::debug!("picking last point from dependency");
        let last_point = data
            .into_iter()
            .next_back()
            // `None` from `query_data` means that there is absolutely no data
            // in the dependency, which in all (current) cases means that
            // the value is 0
            .unwrap_or(TimespanValue::<Resolution, Value>::with_zero_value(
                Resolution::from_date(cx.time.date_naive()),
            ));
        Ok(last_point)
    }
}
