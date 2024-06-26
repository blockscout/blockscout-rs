//! The opposite of [cumulative chart](crate::data_source::kinds::local_db::CumulativeLocalDbChartSource).
//! However, it can also work on remotely stored sources, thus different location of the type.
//!
//! I.e. chart "New accounts" is a delta of  "Total accounts".

use std::{
    fmt::Display,
    marker::PhantomData,
    ops::{RangeInclusive, SubAssign},
    str::FromStr,
};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::prelude::Zero;
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    charts::Point,
    data_processing::deltas,
    data_source::{DataSource, UpdateContext},
    UpdateError,
};

/// Calculate delta data from cumulative dependency.
///
/// Missing points in dependency's output are expected to mean "the value is the
/// same as in previous point" (==`MissingDatePolicy::FillPrevious`).
/// [see "Dependency requirements" here](crate::data_source::kinds)
///
/// See [module-level documentation](self) for more details.
pub struct Delta<D>(PhantomData<D>)
where
    D: DataSource;

impl<D, DV> DataSource for Delta<D>
where
    D: DataSource<Output = Vec<DV>>,
    DV: Point + Default,
    DV::Value: FromStr + SubAssign + Zero + Clone + Display,
    <DV::Value as FromStr>::Err: Display,
{
    type MainDependencies = D;
    type ResolutionDependencies = ();
    type Output = Vec<DV>;
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
        range: Option<RangeInclusive<DateTimeUtc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        let request_range = range.clone().map(|r| {
            let start = r
                .start()
                .checked_sub_signed(TimeDelta::days(1))
                .unwrap_or(DateTime::<Utc>::MAX_UTC);
            let end = *r.end();
            start..=end
        });
        let cum_data: Vec<DV> =
            D::query_data(cx, request_range, dependency_data_fetch_timer).await?;
        let (prev_value, cum_data) = if range.is_some() {
            let mut cum_data = cum_data.into_iter();
            let Some(range_start) = cum_data.next() else {
                tracing::warn!("Value before the range was not found, finishing update");
                return Ok(vec![]);
            };
            let range_start_value = range_start.into_parts().1;
            tracing::debug!(
                range_start_value = %range_start_value,
                cumulative_points_len = cum_data.len(),
                "calculating deltas from cumulative"
            );
            (range_start_value, cum_data.collect())
        } else {
            (DV::Value::zero(), cum_data)
        };
        tracing::debug!(
            prev_value = %prev_value,
            cumulative_points_len = cum_data.len(),
            "calculating deltas from cumulative"
        );
        deltas::<DV>(cum_data, prev_value)
    }
}
