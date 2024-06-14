//! Sum data source.
//!
//! Sums all points from the other (vector) source.

use std::{
    marker::PhantomData,
    ops::{AddAssign, RangeInclusive},
};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::Zero;
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    charts::db_interaction::types::DateValue,
    data_processing::sum,
    data_source::{source::DataSource, UpdateContext},
    UpdateError,
};

struct Sum<D>(PhantomData<D>)
where
    D: DataSource;

impl<D, DV> DataSource for Sum<D>
where
    D: DataSource<Output = Vec<DV>>,
    DV: DateValue + Send,
    DV::Value: AddAssign + Clone + Zero,
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
        // it's possible to not request full data range and use last value; can be updated
        // similarly to cumulative
        let full_data: Vec<_> = D::query_data(cx, None, dependency_data_fetch_timer)
            .await?
            .into();
        tracing::debug!(points_len = full_data.len(), "calculating sum");
        let zero = <DV as DateValue>::Value::zero();
        let sum = sum::<DV>(&full_data, zero)?;
        Ok(sum)
    }
}
