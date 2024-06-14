use std::ops::RangeInclusive;

use blockscout_metrics_tools::AggregateTimer;
use chrono::Utc;
use futures::Future;
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    charts::db_interaction::write::set_last_updated_at,
    data_source::{DataSource, UpdateContext},
    DateValueString, UpdateError,
};

/// In most cases, [`super::DefaultCreate`] is enough.
pub trait CreateBehaviour {
    /// Create chart in db. Should not overwrite existing data.
    fn create(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> impl Future<Output = Result<(), DbErr>> + Send;
}

pub trait UpdateBehaviour<MainDep, ResolutionDep>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
{
    /// Update only chart values.
    ///
    /// `dependency_data_fetch_timer` - timer to track data fetch from (remote) dependencies.
    /// `min_blockscout_block` - indicator of blockscout reindexation
    fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        last_accurate_point: Option<DateValueString>,
        min_blockscout_block: i64,
        dependency_data_fetch_timer: &mut AggregateTimer,
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
}

/// In most cases, [`super::DefaultQueryVec`] is enough.
pub trait QueryBehaviour {
    /// Currently `P` or `Vec<P>`, where `P` is some type representing a data point
    type Output: Send;

    /// Retrieve chart data from (local) storage.
    fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<RangeInclusive<DateTimeUtc>>,
    ) -> impl std::future::Future<Output = Result<Self::Output, UpdateError>> + std::marker::Send;
}
