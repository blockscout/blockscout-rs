use std::{marker::PhantomData, ops::RangeInclusive, time::Duration};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{NaiveDate, Utc};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    charts::{
        chart::{chart_portrait, ChartData},
        create_chart,
        db_interaction::{
            chart_updaters::common_operations::{self, get_min_block_blockscout, get_nth_last_row},
            read::get_chart_metadata,
        },
    },
    data_source::{
        source::DataSource,
        source_metrics::DataSourceMetrics,
        types::{UpdateContext, UpdateParameters},
    },
    get_chart_data, metrics, Chart, DateValue, UpdateError,
};

// todo: instruction on how to implement
pub trait UpdateableChart: Chart {
    type PrimaryDependency: DataSource;
    type SecondaryDependencies: DataSource;

    /// Create chart in db. Does not overwrite existing data.
    fn create(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<(), DbErr>> + Send {
        async move { create_chart(db, Self::NAME.into(), Self::chart_type(), init_time).await }
    }

    /// Update chart data (values + metadata).
    ///
    /// Should be idempontent with regards to `current_time` (in `cx`).
    /// It is a normal behaviour to call this method within single update
    /// (= with same `current_time`).
    async fn update(
        cx: &UpdateContext<UpdateParameters<'_>>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let metadata = get_chart_metadata(cx.user_context.db, Self::NAME).await?;
        if let Some(last_updated_at) = metadata.last_updated_at {
            if cx.user_context.current_time == last_updated_at {
                // no need to perform update
                return Ok(());
            }
        }
        let chart_id = metadata.id;
        let min_blockscout_block = get_min_block_blockscout(cx.user_context.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let offset = Some(Self::approximate_trailing_points());
        let last_updated_row = get_nth_last_row::<Self>(
            chart_id,
            min_blockscout_block,
            cx.user_context.db,
            cx.user_context.force_full,
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
        Self::update_metadata(cx.user_context.db, chart_id, cx.user_context.current_time).await?;
        Ok(())
    }

    /// Update only chart values.
    async fn update_values(
        cx: &UpdateContext<UpdateParameters<'_>>,
        chart_id: i32,
        update_from_row: Option<DateValue>,
        min_blockscout_block: i64,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError>;

    /// Update only chart metadata.
    async fn update_metadata(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: chrono::DateTime<Utc>,
    ) -> Result<(), UpdateError> {
        common_operations::set_last_updated_at(chart_id, db, update_time)
            .await
            .map_err(UpdateError::StatsDB)
    }

    /// Retrieve chart data from (local) storage.
    async fn query_data(
        cx: &UpdateContext<UpdateParameters<'_>>,
        range: std::ops::RangeInclusive<sea_orm::prelude::Date>,
    ) -> Result<ChartData, UpdateError> {
        let values = get_chart_data(
            cx.user_context.db,
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
        let metadata = get_chart_metadata(cx.user_context.db, Self::NAME).await?;
        Ok(ChartData { metadata, values })
    }
}

/// Wrapper struct used for avoiding implementation conflicts
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

    async fn update_from_remote(
        cx: &UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError> {
        Self::PrimaryDependency::update_from_remote(cx).await?;
        Self::SecondaryDependencies::update_from_remote(cx).await?;
        let mut remote_fetch_timer = AggregateTimer::new();
        let _update_timer = metrics::CHART_UPDATE_TIME
            .with_label_values(&[Self::NAME])
            .start_timer();

        C::update(cx, &mut remote_fetch_timer)
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
        cx: &UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
        // local data is queried, do not track in remote timer
        _remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<ChartData, UpdateError> {
        C::query_data(cx, range).await
    }

    async fn init_itself(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        C::create(db, init_time).await
    }
}
