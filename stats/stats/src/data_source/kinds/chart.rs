use std::{marker::PhantomData, ops::RangeInclusive};

use chrono::{NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DatabaseConnection, DbErr, QuerySelect};

use crate::{
    charts::{
        chart::{chart_portrait, ChartData, ChartMetadata},
        create_chart,
        db_interaction::chart_updaters::common_operations::{
            self, get_min_block_blockscout, get_nth_last_row,
        },
        find_chart,
    },
    data_source::{
        source_trait::DataSource,
        types::{UpdateContext, UpdateParameters},
    },
    get_chart_data, Chart, DateValue, MissingDatePolicy, ReadError, UpdateError,
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

    async fn update(cx: &UpdateContext<UpdateParameters<'_>>) -> Result<(), UpdateError> {
        let chart_id = Self::query_chart_id(cx.user_context.db)
            .await?
            .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
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
        Self::update_values(cx, chart_id, last_updated_row, min_blockscout_block).await?;
        Self::update_metadata(cx.user_context.db, chart_id, cx.user_context.current_time).await
    }

    async fn update_values(
        cx: &UpdateContext<UpdateParameters<'_>>,
        chart_id: i32,
        update_from_row: Option<DateValue>,
        min_blockscout_block: i64,
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

    // todo: maybe more to `Chart`??
    async fn query_chart_id(db: &DatabaseConnection) -> Result<Option<i32>, UpdateError> {
        find_chart(db, Self::name())
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
        let chart = entity::charts::Entity::find()
            .column(entity::charts::Column::Id)
            .filter(entity::charts::Column::Name.eq(Self::name()))
            .one(cx.user_context.db)
            .await
            .map_err(ReadError::from)?
            .ok_or_else(|| ReadError::NotFound(Self::name().into()))?;
        let metadata = ChartMetadata {
            last_update: chart
                .last_updated_at
                .ok_or_else(|| ReadError::NotFound(Self::name().into()))?
                .with_timezone(&Utc),
        };
        Ok(ChartData { metadata, values })
    }
}

pub struct UpdateableChartWrapper<T: UpdateableChart>(PhantomData<T>);

#[portrait::fill(portrait::delegate(T))]
impl<T: UpdateableChart + Chart> Chart for UpdateableChartWrapper<T> {}

impl<C: UpdateableChart> DataSource for UpdateableChartWrapper<C> {
    type PrimaryDependency = C::PrimaryDependency;
    type SecondaryDependencies = C::SecondaryDependencies;
    type Output = ChartData;

    async fn update_from_remote(
        cx: &UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError> {
        Self::PrimaryDependency::update_from_remote(cx).await?;
        Self::SecondaryDependencies::update_from_remote(cx).await?;
        C::update(cx).await?;
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
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
