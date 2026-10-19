// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{future::Future, marker::Send};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    ChartError, RequestedPointsLimit,
    charts::db_interaction::write::set_last_updated_at,
    data_source::{DataSource, UpdateContext},
    range::UniversalRange,
    types::TimespanValue,
};

/// In most cases, [`super::DefaultCreate`] is enough.
pub trait CreateBehaviour {
    /// Create chart in db. Should not overwrite existing data.
    fn create(
        db: &DatabaseConnection,
        init_time: &DateTime<Utc>,
    ) -> impl Future<Output = Result<(), DbErr>> + Send;
}

// generic parameters are to ensure that dependencies in implementations
// of this trait match dependencies in `impl DataSource for LocalDbChartSource<..>`
pub trait UpdateBehaviour<MainDep, ResolutionDep, Resolution>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
{
    /// Update only chart values.
    ///
    /// `dependency_data_fetch_timer` - timer to track data fetch from (remote) dependencies.
    /// `min_indexer_block` - indicator of blockscout reindexation
    fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        last_accurate_point: Option<TimespanValue<Resolution, String>>,
        min_indexer_block: i64,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> impl Future<Output = Result<(), ChartError>> + Send;

    /// Update only chart metadata.
    fn update_metadata(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), ChartError>> + Send {
        async move {
            set_last_updated_at(chart_id, db, update_time)
                .await
                .map_err(ChartError::StatsDB)
        }
    }

    /// Probes whether `MainDep` has at least one point of data in `range`,
    /// without updating anything. Used only by the interchain trigger-2 gap
    /// probe (`local_db::interchain_history_gap_has_data`), which is gated to
    /// day-resolution line charts — i.e. this is only ever actually reached
    /// where `Self = BatchUpdate<...>`, which overrides it with a real probe
    /// (`MainDep::query_data` over `range`, checked for emptiness — exactly
    /// what a real batch step fetches).
    ///
    /// The default here (used by `PassPoint`, i.e. counters) is never
    /// actually reached in practice, since counters are never day-resolution
    /// *line* charts. `Ok(true)` ("has data", i.e. never suppress a rebuild
    /// on this account) is the safe choice should that assumption ever stop
    /// holding — it fails toward an extra rebuild, not a missed one.
    fn probe_gap_has_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> impl Future<Output = Result<bool, ChartError>> + Send {
        async move {
            let _ = (cx, range, dependency_data_fetch_timer);
            Ok(true)
        }
    }
}

/// In most cases, [`super::DefaultQueryVec`] is enough.
pub trait QueryBehaviour {
    /// Currently `P` or `Vec<P>`, where `P` is some type representing a data point
    type Output: Send;

    /// Retrieve chart data from local storage.
    fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        points_limit: Option<RequestedPointsLimit>,
        fill_missing_dates: bool,
    ) -> impl Future<Output = Result<Self::Output, ChartError>> + Send;
}
