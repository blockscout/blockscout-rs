//! Source that is persisted in local database.
//!
//! Such sources are the only ones (so far) that
//! change their state during update.
//! For example, remote sources are updated independently from
//! this service, and sources from data manipulation only transform
//! some other source's data on query.
//!
//! Charts are intended to be such persisted sources,
//! because their data is directly retreived from the database (on requests).

use std::{marker::PhantomData, ops::Range, time::Duration};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use parameter_traits::{CreateBehaviour, QueryBehaviour, UpdateBehaviour};
use parameters::{
    update::{
        batching::{
            parameters::{AddLastValueStep, Batch30Days, PassVecStep},
            BatchUpdate,
        },
        point::PassPoint,
    },
    DefaultCreate, DefaultQueryLast, DefaultQueryVec,
};
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    charts::{
        chart_properties_portrait,
        db_interaction::read::{get_chart_metadata, get_min_block_blockscout, last_accurate_point},
        ChartProperties, Named,
    },
    data_source::{DataSource, UpdateContext},
    metrics, UpdateError,
};

use super::auxiliary::PartialCumulative;

pub mod parameter_traits;
pub mod parameters;

/// The source is configurable in many aspects. In particular,
/// - dependencies
/// - implementation of CRUD (without D) (=behaviour)
/// - chart settings/properties
///
///
/// There are types that implement each of the behaviour type in
/// [`parameters`]; also there are type aliases in [`self`]
/// with common parameter combinations.
///
/// See [module-level documentation](self) for more details.
pub struct LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>(
    PhantomData<(MainDep, ResolutionDep, Create, Update, Query, ChartProps)>,
)
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    Create: CreateBehaviour,
    Update: UpdateBehaviour<MainDep, ResolutionDep>,
    Query: QueryBehaviour,
    ChartProps: ChartProperties;

/// Chart with default create/query and batch update with configurable step logic
pub type BatchLocalDbChartSourceWithDefaultParams<MainDep, ResolutionDep, BatchStep, ChartProps> =
    LocalDbChartSource<
        MainDep,
        ResolutionDep,
        DefaultCreate<ChartProps>,
        BatchUpdate<
            MainDep,
            ResolutionDep,
            BatchStep,
            Batch30Days,
            DefaultQueryVec<ChartProps>,
            ChartProps,
        >,
        DefaultQueryVec<ChartProps>,
        ChartProps,
    >;

// not in `data_manipulation` because it requires retrieving latest (self) value before
// next batch
/// Chart with cumulative data calculated from delta dependency
/// (dependency with changes from previous point == increments+decrements or deltas)
///
/// So, if the values of `NewItemsChart` are [1, 2, 3, 4], then
/// cumulative chart will produce [1, 3, 6, 10].
///
/// Missing points in dependency's output are expected to mean zero value
/// (==`MissingDatePolicy::FillZero`).
/// [see "Dependency requirements" here](crate::data_source::kinds)
///
/// The opposite logic to [`Delta`](`crate::data_source::kinds::data_manipulation::delta::Delta`)
pub type CumulativeLocalDbChartSource<DeltaDep, C> = LocalDbChartSource<
    PartialCumulative<DeltaDep>,
    (),
    DefaultCreate<C>,
    BatchUpdate<
        PartialCumulative<DeltaDep>,
        (),
        AddLastValueStep<C>,
        Batch30Days,
        DefaultQueryVec<C>,
        C,
    >,
    DefaultQueryVec<C>,
    C,
>;

/// Chart that stores vector data received from provided dependency (without
/// any manipulations)
pub type DirectVecLocalDbChartSource<Dependency, C> = LocalDbChartSource<
    Dependency,
    (),
    DefaultCreate<C>,
    BatchUpdate<Dependency, (), PassVecStep, Batch30Days, DefaultQueryVec<C>, C>,
    DefaultQueryVec<C>,
    C,
>;

/// Chart that stores single data point received from provided dependency (without
/// any manipulations)
pub type DirectPointLocalDbChartSource<Dependency, C> = LocalDbChartSource<
    Dependency,
    (),
    DefaultCreate<C>,
    PassPoint<Dependency>,
    DefaultQueryLast<C>,
    C,
>;

impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
    LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    Create: CreateBehaviour,
    Update: UpdateBehaviour<MainDep, ResolutionDep>,
    Query: QueryBehaviour,
    ChartProps: ChartProperties,
{
    /// Performs common checks and prepares values useful for further
    /// update. Then proceeds to update according to parameters.
    async fn update_itself(
        cx: &UpdateContext<'_>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let metadata = get_chart_metadata(cx.db, ChartProps::NAME).await?;
        if let Some(last_updated_at) = metadata.last_updated_at {
            if cx.time == last_updated_at {
                // no need to perform update.
                // mostly catches second call to update e.g. when both
                // dependency and this source are in one group and enabled.
                tracing::info!(
                    "Not updating the chart because it was already handled within ongoing update"
                );
                return Ok(());
            }
        }
        let chart_id = metadata.id;
        let min_blockscout_block = get_min_block_blockscout(cx.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let offset = Some(ChartProps::approximate_trailing_points());
        let last_accurate_point = last_accurate_point::<ChartProps>(
            chart_id,
            min_blockscout_block,
            cx.db,
            cx.force_full,
            offset,
        )
        .await?;
        tracing::info!(last_accurate_point =? last_accurate_point, chart_name = ChartProps::NAME, "updating chart values");
        Update::update_values(
            cx,
            chart_id,
            last_accurate_point,
            min_blockscout_block,
            dependency_data_fetch_timer,
        )
        .await?;
        tracing::info!(chart_name = ChartProps::NAME, "updating chart metadata");
        Update::update_metadata(cx.db, chart_id, cx.time).await?;
        Ok(())
    }

    fn observe_query_time(time: Duration) {
        if time > Duration::ZERO {
            metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[Self::NAME])
                .observe(time.as_secs_f64());
        }
    }
}

impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps> DataSource
    for LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    Create: CreateBehaviour,
    Update: UpdateBehaviour<MainDep, ResolutionDep>,
    Query: QueryBehaviour,
    ChartProps: ChartProperties,
{
    type MainDependencies = MainDep;
    type ResolutionDependencies = ResolutionDep;
    type Output = Query::Output;

    const MUTEX_ID: Option<&'static str> = Some(ChartProps::NAME);

    async fn init_itself(db: &DatabaseConnection, init_time: &DateTime<Utc>) -> Result<(), DbErr> {
        Create::create(db, init_time).await
    }

    async fn update_itself(cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        // set up metrics + write some logs

        let mut dependency_data_fetch_timer = AggregateTimer::new();
        let _update_timer = metrics::CHART_UPDATE_TIME
            .with_label_values(&[ChartProps::NAME])
            .start_timer();
        tracing::info!(chart = ChartProps::NAME, "started chart update");

        Self::update_itself(cx, &mut dependency_data_fetch_timer)
            .await
            .inspect_err(|err| {
                metrics::UPDATE_ERRORS
                    .with_label_values(&[ChartProps::NAME])
                    .inc();
                tracing::error!(
                    chart = ChartProps::NAME,
                    "error during updating chart: {}",
                    err
                );
            })?;

        Self::observe_query_time(dependency_data_fetch_timer.total_time());
        tracing::info!(chart = ChartProps::NAME, "successfully updated chart");
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        let _timer = dependency_data_fetch_timer.start_interval();
        Query::query_data(cx, range).await
    }
}

// need to delegate these traits for update groups to use

impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps> Named
    for LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    Create: CreateBehaviour,
    Update: UpdateBehaviour<MainDep, ResolutionDep>,
    Query: QueryBehaviour,
    ChartProps: ChartProperties + Named,
{
    const NAME: &'static str = ChartProps::NAME;
}

#[portrait::fill(portrait::delegate(ChartProps))]
impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps> ChartProperties
    for LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep> + Sync,
    Query: QueryBehaviour + Sync,
    ChartProps: ChartProperties,
{
}
