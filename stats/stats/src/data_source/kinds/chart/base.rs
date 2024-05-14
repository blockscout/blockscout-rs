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
        find_chart,
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
        async move { create_chart(db, Self::name().into(), Self::chart_type(), init_time).await }
    }

    async fn update(
        cx: &UpdateContext<UpdateParameters<'_>>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let metadata = get_chart_metadata(cx.user_context.db, Self::name()).await?;
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

    async fn update_values(
        cx: &UpdateContext<UpdateParameters<'_>>,
        chart_id: i32,
        update_from_row: Option<DateValue>,
        min_blockscout_block: i64,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError>;

    async fn update_metadata(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: chrono::DateTime<Utc>,
    ) -> Result<(), UpdateError> {
        common_operations::set_last_updated_at(chart_id, db, update_time)
            .await
            .map_err(UpdateError::StatsDB)
    }

    async fn query_data(
        cx: &UpdateContext<UpdateParameters<'_>>,
        range: std::ops::RangeInclusive<sea_orm::prelude::Date>,
    ) -> Result<ChartData, UpdateError> {
        let values = get_chart_data(
            cx.user_context.db,
            Self::name(),
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
        let metadata = get_chart_metadata(cx.user_context.db, Self::name()).await?;
        Ok(ChartData { metadata, values })
    }
}

pub struct UpdateableChartWrapper<C: UpdateableChart>(PhantomData<C>);

#[portrait::fill(portrait::delegate(C))]
impl<C: UpdateableChart + Chart> Chart for UpdateableChartWrapper<C> {}

impl<C: UpdateableChart> DataSourceMetrics for UpdateableChartWrapper<C> {
    fn observe_query_time(time: std::time::Duration) {
        if time > Duration::ZERO {
            metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[Self::name()])
                .observe(time.as_secs_f64());
        }
    }
}

impl<C: UpdateableChart> DataSource for UpdateableChartWrapper<C> {
    type PrimaryDependency = C::PrimaryDependency;
    type SecondaryDependencies = C::SecondaryDependencies;
    type Output = ChartData;

    async fn update_from_remote(
        cx: &UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError> {
        Self::PrimaryDependency::update_from_remote(cx).await?;
        Self::SecondaryDependencies::update_from_remote(cx).await?;
        let mut remote_fetch_timer = AggregateTimer::new();

        C::update(cx, &mut remote_fetch_timer).await?;

        Self::observe_query_time(remote_fetch_timer.total_time());
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
