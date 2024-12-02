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

use std::{fmt::Debug, marker::PhantomData, time::Duration};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, SubsecRound, Utc};
use parameter_traits::{CreateBehaviour, QueryBehaviour, UpdateBehaviour};
use parameters::{
    update::{
        batching::{
            parameters::{AddLastValueStep, Batch30Days, PassVecStep},
            BatchUpdate,
        },
        point::PassPoint,
    },
    DefaultCreate, DefaultQueryLast, DefaultQueryVec, QueryLastWithEstimationFallback,
};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{
    charts::{
        chart_properties_portrait,
        db_interaction::read::{get_chart_metadata, get_min_block_blockscout, last_accurate_point},
        ChartProperties, Named,
    },
    data_source::{DataSource, UpdateContext},
    metrics,
    range::UniversalRange,
    UpdateError,
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
    pub PhantomData<(MainDep, ResolutionDep, Create, Update, Query, ChartProps)>,
)
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    Create: CreateBehaviour,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution>,
    Query: QueryBehaviour,
    ChartProps: ChartProperties;

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
pub type DailyCumulativeLocalDbChartSource<DeltaDep, C> = LocalDbChartSource<
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
pub type DirectVecLocalDbChartSource<Dependency, BatchSizeUpperBound, C> = LocalDbChartSource<
    Dependency,
    (),
    DefaultCreate<C>,
    BatchUpdate<Dependency, (), PassVecStep, BatchSizeUpperBound, DefaultQueryVec<C>, C>,
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

pub type DirectPointLocalDbChartSourceWithEstimate<Dependency, Estimate, C> = LocalDbChartSource<
    Dependency,
    (),
    DefaultCreate<C>,
    PassPoint<Dependency>,
    QueryLastWithEstimationFallback<Estimate, C>,
    C,
>;

impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
    LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution> + Sync,
    Query: QueryBehaviour + Sync,
    ChartProps: ChartProperties,
    ChartProps::Resolution: Ord + Clone + Debug,
{
    /// `new` function that is created solely for the purposes of
    /// dynamic dispatch (see where it's used).
    pub fn new_for_dynamic_dispatch() -> Self {
        Self(PhantomData)
    }

    /// Performs common checks and prepares values useful for further
    /// update. Then proceeds to update according to parameters.
    async fn update_itself_inner(
        cx: &UpdateContext<'_>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let metadata = get_chart_metadata(cx.db, &ChartProps::key()).await?;
        if let Some(last_updated_at) = metadata.last_updated_at {
            if postgres_timestamps_eq(cx.time, last_updated_at) {
                // no need to perform update.
                // mostly catches second call to update e.g. when both
                // dependency and this source are in one group and enabled.
                tracing::debug!(
                    last_updated_at =? last_updated_at,
                    update_timestamp =? cx.time,
                    "Not updating the chart because it was already handled within ongoing update"
                );
                return Ok(());
            } else {
                tracing::debug!(
                    last_updated_at =? last_updated_at,
                    update_timestamp =? cx.time,
                    "Performing an update"
                );
            }
        }
        let chart_id = metadata.id;
        let min_blockscout_block = get_min_block_blockscout(cx.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let last_accurate_point = last_accurate_point::<ChartProps, Query>(
            chart_id,
            min_blockscout_block,
            cx.db,
            cx.force_full,
            ChartProps::approximate_trailing_points(),
            ChartProps::missing_date_policy(),
        )
        .await?;
        tracing::info!(last_accurate_point =? last_accurate_point, chart =% ChartProps::key(), "updating chart values");
        Update::update_values(
            cx,
            chart_id,
            last_accurate_point,
            min_blockscout_block,
            dependency_data_fetch_timer,
        )
        .await?;
        tracing::info!(chart =% ChartProps::key(), "updating chart metadata");
        Update::update_metadata(cx.db, chart_id, cx.time).await?;
        Ok(())
    }

    fn observe_query_time(time: Duration) {
        if time > Duration::ZERO {
            metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[&ChartProps::key().to_string()])
                .observe(time.as_secs_f64());
        }
    }
}

/// Compare timestamps as they're seen in Postgres (compare up to microseconds)
fn postgres_timestamps_eq(time_1: DateTime<Utc>, time_2: DateTime<Utc>) -> bool {
    // PostgreSQL stores timestamps with microsecond precision
    // therefore, we need to drop any values smaller than microsecond
    // microsecond = 10^(-6) => compare up to 6 digits after comma
    time_1.trunc_subsecs(6).eq(&time_2.trunc_subsecs(6))
}

impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps> DataSource
    for LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution> + Sync,
    Query: QueryBehaviour + Sync,
    ChartProps: ChartProperties,
    ChartProps::Resolution: Ord + Clone + Debug + Send,
{
    type MainDependencies = MainDep;
    type ResolutionDependencies = ResolutionDep;
    type Output = Query::Output;

    fn mutex_id() -> Option<String> {
        Some(ChartProps::key().into())
    }

    async fn init_itself(db: &DatabaseConnection, init_time: &DateTime<Utc>) -> Result<(), DbErr> {
        Create::create(db, init_time).await
    }

    async fn update_itself(cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        // set up metrics + write some logs

        let mut dependency_data_fetch_timer = AggregateTimer::new();
        let _update_timer = metrics::CHART_UPDATE_TIME
            .with_label_values(&[&ChartProps::key().to_string()])
            .start_timer();
        tracing::info!(chart =% ChartProps::key(), "started chart update");

        Self::update_itself_inner(cx, &mut dependency_data_fetch_timer)
            .await
            .inspect_err(|err| {
                metrics::UPDATE_ERRORS
                    .with_label_values(&[&ChartProps::key().to_string()])
                    .inc();
                tracing::error!(
                    chart =% ChartProps::key(),
                    "error during updating chart: {}",
                    err
                );
            })?;

        Self::observe_query_time(dependency_data_fetch_timer.total_time());
        tracing::info!(chart =% ChartProps::key(), "successfully updated chart");
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        let _timer = dependency_data_fetch_timer.start_interval();
        // maybe add `fill_missing_dates` parameter to current function as well in the future
        // to get rid of "Note" in the `DataSource`'s method documentation
        Query::query_data(cx, range, None, false).await
    }
}

// need to delegate these traits for update groups to use

impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps> Named
    for LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    Create: CreateBehaviour,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution>,
    Query: QueryBehaviour,
    ChartProps: ChartProperties + Named,
{
    fn name() -> String {
        ChartProps::name()
    }
}

#[portrait::fill(portrait::delegate(ChartProps))]
impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps> ChartProperties
    for LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution> + Sync,
    Query: QueryBehaviour + Sync,
    ChartProps: ChartProperties,
{
}

#[cfg(test)]
mod tests {
    mod update_itself_is_triggered_once_per_group {
        use std::{
            collections::HashSet,
            ops::DerefMut,
            str::FromStr,
            sync::{Arc, OnceLock},
        };

        use blockscout_metrics_tools::AggregateTimer;
        use chrono::{DateTime, Days, NaiveDate, TimeDelta, Utc};
        use entity::sea_orm_active_enums::ChartType;
        use tokio::sync::Mutex;

        use crate::{
            charts::db_interaction::write::insert_data_many,
            construct_update_group,
            data_source::{
                kinds::local_db::{
                    parameter_traits::UpdateBehaviour,
                    parameters::{DefaultCreate, DefaultQueryLast},
                    DirectPointLocalDbChartSource, LocalDbChartSource,
                },
                types::{BlockscoutMigrations, Get},
                DataSource, UpdateContext, UpdateParameters,
            },
            gettable_const,
            tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
            types::{timespans::DateValue, TimespanValue},
            update_group::{SyncUpdateGroup, UpdateGroup},
            ChartProperties, Named, UpdateError,
        };

        type WasTriggeredStorage = Arc<Mutex<bool>>;

        // `OnceLock` in order to return the same instance each time
        static FLAG: OnceLock<WasTriggeredStorage> = OnceLock::new();

        gettable_const!(WasTriggered: WasTriggeredStorage = FLAG.get_or_init(|| Arc::new(Mutex::new(false))).clone());

        struct UpdateSingleTriggerAsserter;

        impl UpdateSingleTriggerAsserter {
            pub async fn record_trigger() {
                let mut was_triggered_guard = WasTriggered::get().lock_owned().await;
                let was_triggered = was_triggered_guard.deref_mut();
                assert!(!*was_triggered, "update triggered twice");
                *was_triggered = true;
            }

            pub async fn reset_triggers() {
                let mut was_triggered_guard = WasTriggered::get().lock_owned().await;
                let was_triggered = was_triggered_guard.deref_mut();
                *was_triggered = false;
            }
        }

        impl<M, R, Resolution> UpdateBehaviour<M, R, Resolution> for UpdateSingleTriggerAsserter
        where
            M: DataSource,
            R: DataSource,
            Resolution: Send,
        {
            async fn update_values(
                cx: &UpdateContext<'_>,
                chart_id: i32,
                _last_accurate_point: Option<TimespanValue<Resolution, String>>,
                min_blockscout_block: i64,
                _dependency_data_fetch_timer: &mut AggregateTimer,
            ) -> Result<(), UpdateError> {
                Self::record_trigger().await;
                // insert smth for dependency to work well
                let data = DateValue::<String> {
                    timespan: cx.time.date_naive(),
                    value: "0".to_owned(),
                };
                let value = data.active_model(chart_id, Some(min_blockscout_block));
                insert_data_many(cx.db, vec![value])
                    .await
                    .map_err(UpdateError::StatsDB)?;
                Ok(())
            }
        }

        struct TestedChartProps;

        impl Named for TestedChartProps {
            fn name() -> String {
                "double_update_tested_chart".into()
            }
        }

        impl ChartProperties for TestedChartProps {
            type Resolution = NaiveDate;

            fn chart_type() -> ChartType {
                ChartType::Counter
            }
        }

        type TestedChart = LocalDbChartSource<
            (),
            (),
            DefaultCreate<TestedChartProps>,
            UpdateSingleTriggerAsserter,
            DefaultQueryLast<TestedChartProps>,
            TestedChartProps,
        >;

        struct ChartDependedOnTestedProps;

        impl Named for ChartDependedOnTestedProps {
            fn name() -> String {
                "double_update_dependant_chart".into()
            }
        }

        impl ChartProperties for ChartDependedOnTestedProps {
            type Resolution = NaiveDate;

            fn chart_type() -> ChartType {
                ChartType::Counter
            }
        }

        type ChartDependedOnTested =
            DirectPointLocalDbChartSource<TestedChart, ChartDependedOnTestedProps>;

        construct_update_group!(TestUpdateGroup {
            charts: [TestedChart, ChartDependedOnTested]
        });

        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn update_itself_is_triggered_once_per_group() {
            let _ = tracing_subscriber::fmt::try_init();
            let (db, blockscout) = init_db_all("update_itself_is_triggered_once_per_group").await;
            let current_time = DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
            let current_date = current_time.date_naive();
            fill_mock_blockscout_data(&blockscout, current_date).await;
            let enabled = HashSet::from(
                [TestedChartProps::key(), ChartDependedOnTestedProps::key()].map(|l| l.to_owned()),
            );
            let mutexes = TestUpdateGroup
                .list_dependency_mutex_ids()
                .into_iter()
                .map(|id| (id.to_owned(), Arc::new(Mutex::new(()))))
                .collect();
            let group = SyncUpdateGroup::new(&mutexes, Arc::new(TestUpdateGroup)).unwrap();
            group
                .create_charts_with_mutexes(&db, Some(current_time), &enabled)
                .await
                .unwrap();

            let next_time = current_time.checked_add_days(Days::new(1)).unwrap();
            let parameters = UpdateParameters {
                db: &db,
                blockscout: &blockscout,
                blockscout_applied_migrations: BlockscoutMigrations::latest(),
                update_time_override: Some(next_time),
                force_full: true,
            };
            group
                .update_charts_with_mutexes(parameters, &enabled)
                .await
                .unwrap();

            UpdateSingleTriggerAsserter::reset_triggers().await;

            let next_next_time = next_time.checked_add_days(Days::new(1)).unwrap();
            // it also works with high-precision timestamps
            //
            // regression: had a bug where due to postgres having resolution of 1 microsecond stored a different
            // timestamp to the one provided
            let time = next_next_time + TimeDelta::nanoseconds(1);
            let parameters = UpdateParameters {
                db: &db,
                blockscout: &blockscout,
                blockscout_applied_migrations: BlockscoutMigrations::latest(),
                update_time_override: Some(time),
                force_full: true,
            };
            group
                .update_charts_with_mutexes(parameters, &enabled)
                .await
                .unwrap();

            UpdateSingleTriggerAsserter::reset_triggers().await;

            // also test if there is any rounding when inserting metadata
            let time = next_next_time + TimeDelta::nanoseconds(500);
            let parameters = UpdateParameters {
                db: &db,
                blockscout: &blockscout,
                blockscout_applied_migrations: BlockscoutMigrations::latest(),
                update_time_override: Some(time),
                force_full: true,
            };
            group
                .update_charts_with_mutexes(parameters, &enabled)
                .await
                .unwrap();

            // also test if there is any rounding when inserting metadata
            let time = next_next_time + TimeDelta::nanoseconds(999);
            let parameters = UpdateParameters {
                db: &db,
                blockscout: &blockscout,
                blockscout_applied_migrations: BlockscoutMigrations::latest(),
                update_time_override: Some(time),
                force_full: true,
            };
            group
                .update_charts_with_mutexes(parameters, &enabled)
                .await
                .unwrap();
        }
    }
}
