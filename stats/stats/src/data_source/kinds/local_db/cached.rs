use std::{fmt::Debug, marker::PhantomData};

use super::parameter_traits::{CreateBehaviour, QueryBehaviour, UpdateBehaviour};
use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    ChartError, ChartKey, ChartProperties, IndexingStatus,
    chart_prelude::{Get, LocalDbChartSource},
    charts::{Named, chart_properties_portrait, db_interaction::read::get_chart_metadata},
    data_source::{DataSource, UpdateContext},
    range::UniversalRange,
};

/// Chart that queries data from remote database when queried
/// but caches the result in local database if it was updated within the last `CacheTimeout`.
///
/// Should only be used for very quick queries because querying local chart will be blocked
/// by query to remote database.
pub struct RemoteCachedLocalDbChartSource<
    MainDep,
    ResolutionDep,
    Create,
    Update,
    Query,
    CacheTimeout,
    ChartProps,
>(
    pub  PhantomData<(
        MainDep,
        ResolutionDep,
        Create,
        Update,
        Query,
        CacheTimeout,
        ChartProps,
    )>,
);

impl<MainDep, ResolutionDep, Create, Update, Query, CacheTimeout, ChartProps>
    RemoteCachedLocalDbChartSource<
        MainDep,
        ResolutionDep,
        Create,
        Update,
        Query,
        CacheTimeout,
        ChartProps,
    >
where
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution> + Sync,
    Query: QueryBehaviour + Sync,
    CacheTimeout: Get<Value = Duration> + Sync,
    ChartProps: ChartProperties,
    ChartProps::Resolution: Ord + Clone + Debug + Send,
{
    pub async fn update_cache_if_needed(
        cx: &UpdateContext<'_>,
        dependency_data_fetch_timer: Option<&mut AggregateTimer>,
    ) -> Result<(), ChartError> {
        let metadata = get_chart_metadata(cx.stats_db, &ChartProps::key()).await?;
        if let Some(last_updated_at) = metadata.last_updated_at
            && cx.time >= last_updated_at + CacheTimeout::get()
        {
            let timer = dependency_data_fetch_timer.map(|timer| timer.start_interval());
            // "cache" is no longer valid
            LocalDbChartSource::<MainDep, ResolutionDep, Create, Update, Query, ChartProps>::update_recursively(cx).await?;
            if let Some(timer) = timer {
                timer.finish();
            }
        }
        Ok(())
    }
}

impl<MainDep, ResolutionDep, Create, Update, Query, CacheTimeout, ChartProps> DataSource
    for RemoteCachedLocalDbChartSource<
        MainDep,
        ResolutionDep,
        Create,
        Update,
        Query,
        CacheTimeout,
        ChartProps,
    >
where
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution> + Sync,
    Query: QueryBehaviour + Sync,
    CacheTimeout: Get<Value = Duration> + Sync,
    ChartProps: ChartProperties,
    ChartProps::Resolution: Ord + Clone + Debug + Send,
{
    type MainDependencies = MainDep;
    type ResolutionDependencies = ResolutionDep;
    type Output = Query::Output;

    fn chart_key() -> Option<ChartKey> {
        LocalDbChartSource::<MainDep, ResolutionDep, Create, Update, Query, ChartProps>::chart_key()
    }

    fn indexing_status_self_requirement() -> IndexingStatus {
        LocalDbChartSource::<MainDep, ResolutionDep, Create, Update, Query, ChartProps>::indexing_status_self_requirement()
    }

    async fn init_itself(db: &DatabaseConnection, init_time: &DateTime<Utc>) -> Result<(), DbErr> {
        LocalDbChartSource::<MainDep, ResolutionDep, Create, Update, Query, ChartProps>::init_itself(db, init_time).await
    }

    async fn update_itself(cx: &UpdateContext<'_>) -> Result<(), ChartError> {
        LocalDbChartSource::<MainDep, ResolutionDep, Create, Update, Query, ChartProps>::update_itself(cx).await
    }

    async fn set_next_update_from_itself(
        db: &DatabaseConnection,
        update_from: chrono::NaiveDate,
    ) -> Result<(), ChartError> {
        LocalDbChartSource::<MainDep, ResolutionDep, Create, Update, Query, ChartProps>::set_next_update_from_itself(db, update_from).await
    }

    fn mutex_id() -> Option<String> {
        LocalDbChartSource::<MainDep, ResolutionDep, Create, Update, Query, ChartProps>::mutex_id()
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        Self::update_cache_if_needed(cx, Some(dependency_data_fetch_timer)).await?;
        // get from "cache"
        LocalDbChartSource::<MainDep, ResolutionDep, Create, Update, Query, ChartProps>::query_data(
            cx,
            range,
            dependency_data_fetch_timer,
        )
        .await
    }
}

// need to delegate these traits for update groups to use

impl<MainDep, ResolutionDep, Create, Update, Query, CacheTimeout, ChartProps> Named
    for RemoteCachedLocalDbChartSource<
        MainDep,
        ResolutionDep,
        Create,
        Update,
        Query,
        CacheTimeout,
        ChartProps,
    >
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    Create: CreateBehaviour,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution>,
    Query: QueryBehaviour,
    CacheTimeout: Get<Value = Duration> + Sync,
    ChartProps: ChartProperties + Named,
{
    fn name() -> String {
        ChartProps::name()
    }
}

#[portrait::fill(portrait::delegate(ChartProps))]
impl<MainDep, ResolutionDep, Create, Update, Query, CacheTimeout, ChartProps> ChartProperties
    for RemoteCachedLocalDbChartSource<
        MainDep,
        ResolutionDep,
        Create,
        Update,
        Query,
        CacheTimeout,
        ChartProps,
    >
where
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution> + Sync,
    Query: QueryBehaviour + Sync,
    CacheTimeout: Get<Value = Duration> + Sync,
    ChartProps: ChartProperties,
{
}
