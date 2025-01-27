//! Batching for update process.
//!
//! Update for some period P can be done only with dependencies'
//! data for the same exact period P.

use std::{fmt::Debug, marker::PhantomData, time::Instant};

use blockscout_metrics_tools::AggregateTimer;
use sea_orm::TransactionTrait;

use crate::{
    charts::db_interaction::write::clear_all_chart_data,
    data_source::{
        kinds::local_db::{parameter_traits::QueryBehaviour, UpdateBehaviour},
        source::DataSource,
        UpdateContext,
    },
    range::UniversalRange,
    types::{ExtendedTimespanValue, Timespan, TimespanValue},
    ChartError, ChartProperties,
};

use super::pass_vec;

pub struct ClearAllAndPassVec<MainDep, Query, ChartProps>(
    PhantomData<(MainDep, Query, ChartProps)>,
)
where
    MainDep: DataSource,
    Query: QueryBehaviour<Output = Vec<ExtendedTimespanValue<ChartProps::Resolution, String>>>,
    ChartProps: ChartProperties;

impl<MainDep, Query, ChartProps> UpdateBehaviour<MainDep, (), ChartProps::Resolution>
    for ClearAllAndPassVec<MainDep, Query, ChartProps>
where
    MainDep: DataSource<Output = Vec<TimespanValue<ChartProps::Resolution, String>>>,
    Query: QueryBehaviour<Output = Vec<ExtendedTimespanValue<ChartProps::Resolution, String>>>,
    ChartProps: ChartProperties,
    ChartProps::Resolution: Timespan + Ord + Clone + Debug + Send + Sync,
{
    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        _last_accurate_point: Option<TimespanValue<ChartProps::Resolution, String>>,
        min_blockscout_block: i64,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), ChartError> {
        let now = Instant::now();
        let db = cx.db.begin().await.map_err(ChartError::StatsDB)?;
        tracing::info!(
            chart =% ChartProps::key(),
            "clearing all data and querying from scratch"
        );
        clear_all_chart_data(&db, chart_id)
            .await
            .map_err(ChartError::StatsDB)?;
        // updating all at once => full range
        let range = UniversalRange::full();
        let main_data = MainDep::query_data(cx, range, dependency_data_fetch_timer).await?;
        let found = pass_vec(&db, chart_id, min_blockscout_block, main_data).await?;
        tracing::info!(
            found =? found,
            elapsed =? now.elapsed(),
            chart =% ChartProps::key(),
            "updated"
        );
        db.commit().await.map_err(ChartError::StatsDB)?;
        Ok(())
    }
}
