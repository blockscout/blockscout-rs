//! Tha most general (so far) trait for charts.
//!
//! It assumes that the chart stores its data via
//! [`UpdateableChart::update_itself`], which can be retrieved
//! with [`UpdateableChart::query_data`].
//!
//! Note that for this chart, `CHART_FETCH_NEW_DATA_TIME` metric
//! is tracked (with a help of `DataSourceMetrics` trait).
//!
//! If you want to define some chart without `UpdateableChart` trait,
//! you might want to handle this metric somehow.

use std::{future::Future, marker::PhantomData, ops::RangeInclusive, time::Duration};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{NaiveDate, Utc};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    charts::{
        chart::{chart_portrait, ChartData},
        db_interaction::{
            read::{get_chart_metadata, get_min_block_blockscout, get_update_start},
            write::{create_chart, set_last_updated_at},
        },
    },
    data_source::{source::DataSource, source_metrics::DataSourceMetrics, types::UpdateContext},
    get_chart_data, metrics, Chart, DateValue, UpdateError,
};

/// See [module-level documentation](self) for details.
pub trait UpdateableChart: Chart {
    type PrimaryDependency: DataSource;
    type SecondaryDependencies: DataSource;

    /// Create chart in db. Does not overwrite existing data.
    fn create(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> impl Future<Output = Result<(), DbErr>> + Send {
        async move { create_chart(db, Self::NAME.into(), Self::chart_type(), init_time).await }
    }

    /// Update this chart data (values + metadata).
    ///
    /// Should be idempontent with regards to `current_time` (in `cx`).
    /// It is a normal behaviour to call this method within single update
    /// (= with same `current_time`).
    fn update_itself(
        cx: &UpdateContext<'_>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> impl Future<Output = Result<(), UpdateError>> + Send {
        async {
            let metadata = get_chart_metadata(cx.db, Self::NAME).await?;
            if let Some(last_updated_at) = metadata.last_updated_at {
                if cx.time == last_updated_at {
                    // no need to perform update
                    return Ok(());
                }
            }
            let chart_id = metadata.id;
            let min_blockscout_block = get_min_block_blockscout(cx.blockscout)
                .await
                .map_err(UpdateError::BlockscoutDB)?;
            let offset = Some(Self::approximate_trailing_points());
            let last_updated_row = get_update_start::<Self>(
                chart_id,
                min_blockscout_block,
                cx.db,
                cx.force_full,
                offset,
            )
            .await?;
            Self::update_values(
                cx,
                chart_id,
                last_updated_row,
                min_blockscout_block,
                remote_fetch_timer,
            )
            .await?;
            Self::update_metadata(cx.db, chart_id, cx.time).await?;
            Ok(())
        }
    }

    /// Update only chart values.
    fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        update_from_row: Option<DateValue>,
        min_blockscout_block: i64,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> impl Future<Output = Result<(), UpdateError>> + Send;

    /// Update only chart metadata.
    fn update_metadata(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: chrono::DateTime<Utc>,
    ) -> impl Future<Output = Result<(), UpdateError>> + Send {
        async move {
            set_last_updated_at(chart_id, db, update_time)
                .await
                .map_err(UpdateError::StatsDB)
        }
    }

    /// Retrieve chart data from (local) storage.
    fn query_data(
        cx: &UpdateContext<'_>,
        range: RangeInclusive<sea_orm::prelude::Date>,
    ) -> impl Future<Output = Result<ChartData, UpdateError>> + Send {
        async move {
            let values: Vec<DateValue> = get_chart_data(
                cx.db,
                Self::NAME,
                Some(*range.start()),
                Some(*range.end()),
                None,
                None,
                Self::approximate_trailing_points(),
            )
            .await?
            .into_iter()
            .map(DateValue::from)
            .collect();
            let metadata = get_chart_metadata(cx.db, Self::NAME).await?;
            Ok(ChartData { metadata, values })
        }
    }
}

/// Wrapper struct used for avoiding implementation conflicts
///
/// See [module-level documentation](self) for details.
pub struct UpdateableChartWrapper<C: UpdateableChart>(PhantomData<C>);

#[portrait::fill(portrait::delegate(C))]
impl<C: UpdateableChart + Chart> Chart for UpdateableChartWrapper<C> {}

impl<C: UpdateableChart> DataSourceMetrics for UpdateableChartWrapper<C> {
    fn observe_query_time(time: std::time::Duration) {
        if time > Duration::ZERO {
            metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[Self::NAME])
                .observe(time.as_secs_f64());
        }
    }
}

impl<C: UpdateableChart> DataSource for UpdateableChartWrapper<C> {
    type PrimaryDependency = C::PrimaryDependency;
    type SecondaryDependencies = C::SecondaryDependencies;
    type Output = ChartData;

    const MUTEX_ID: Option<&'static str> = Some(<C as Chart>::NAME);

    async fn init_itself(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        C::create(db, init_time).await
    }

    async fn update_itself(cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        // data retrieval time
        let mut remote_fetch_timer = AggregateTimer::new();
        let _update_timer = metrics::CHART_UPDATE_TIME
            .with_label_values(&[Self::NAME])
            .start_timer();

        C::update_itself(cx, &mut remote_fetch_timer)
            .await
            .inspect_err(|err| {
                metrics::UPDATE_ERRORS.with_label_values(&[C::NAME]).inc();
                tracing::error!(chart = C::NAME, "error during updating chart: {}", err);
            })?;

        Self::observe_query_time(remote_fetch_timer.total_time());
        tracing::info!(chart = C::NAME, "successfully updated chart");
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: RangeInclusive<NaiveDate>,
        // local data is queried, do not track in remote timer
        _remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<ChartData, UpdateError> {
        C::query_data(cx, range).await
    }
}
