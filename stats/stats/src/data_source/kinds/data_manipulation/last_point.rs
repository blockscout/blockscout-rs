//! Last point data source.
//!
//! Takes last data point from some other (vector) source

use std::{marker::PhantomData, ops::RangeInclusive};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    charts::{
        chart::ChartProperties,
        db_interaction::types::{DateValue, ZeroDateValue},
    },
    data_source::{source::DataSource, UpdateContext},
    utils::day_start,
    MissingDatePolicy, UpdateError,
};

pub struct LastPoint<D>(PhantomData<D>)
where
    D: DataSource;

impl<D, DV> DataSource for LastPoint<D>
where
    D: DataSource<Output = Vec<DV>> + ChartProperties,
    DV: DateValue + ZeroDateValue + Send,
{
    type MainDependencies = D;
    type ResolutionDependencies = ();
    type Output = DV;
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
        _range: Option<RangeInclusive<DateTimeUtc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        let data = D::query_data(
            cx,
            Some(day_start(cx.time.date_naive())..=cx.time),
            dependency_data_fetch_timer,
        )
        .await?;
        tracing::debug!("picking last point from dependency");
        let last_point = data.into_iter().next_back().or_else(|| {
            if D::missing_date_policy() == MissingDatePolicy::FillZero {
                Some(DV::with_zero_value(cx.time.date_naive()))
            } else {
                None
            }
        });
        let last_point = last_point.ok_or(UpdateError::Internal(format!(
            "'{}' returned no data to choose last point from",
            D::NAME
        )))?;
        Ok(last_point)
    }
}
