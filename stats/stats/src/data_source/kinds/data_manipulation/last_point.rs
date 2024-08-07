//! Last point data source.
//!
//! Takes last data point from some other (vector) source

use std::{marker::PhantomData, ops::Range};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    charts::ChartProperties,
    data_source::{source::DataSource, UpdateContext},
    types::{Timespan, TimespanValue, ZeroTimespanValue},
    utils::day_start,
    UpdateError,
};

pub struct LastPoint<DS>(PhantomData<DS>)
where
    DS: DataSource;

impl<DS, Resolution, Value> DataSource for LastPoint<DS>
where
    Resolution: Timespan + Ord + Send,
    Value: Send,
    DS: DataSource<Output = Vec<TimespanValue<Resolution, Value>>> + ChartProperties,
    TimespanValue<Resolution, Value>: ZeroTimespanValue<Resolution>,
{
    type MainDependencies = DS;
    type ResolutionDependencies = ();
    type Output = TimespanValue<Resolution, Value>;
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
        _range: Option<Range<DateTimeUtc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        let data = DS::query_data(
            cx,
            Some(day_start(&cx.time.date_naive())..cx.time),
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
